//! A navigation panel that slides in from the edge of the screen.
//!
//! # Example
//! ```no_run
//! # mod aura { pub mod widget { pub use aura_widget::*; } pub use aura_widget::Renderer; pub use aura_widget::core::*; }
//! # pub type Element<'a, Message> = aura_widget::core::Element<'a, Message, aura_widget::Theme, aura_widget::Renderer>;
//! use aura::widget::{drawer, stack, text};
//!
//! #[derive(Clone)]
//! enum Message {
//!     DrawerOpened,
//!     DrawerDismissed,
//! }
//!
//! # struct State { is_open: bool }
//! fn view(state: &State) -> Element<'_, Message> {
//!     stack![
//!         text("the application"),
//!         drawer(text("navigation"), state.is_open)
//!             .on_open(Message::DrawerOpened)
//!             .on_dismiss(Message::DrawerDismissed),
//!     ]
//!     .into()
//! }
//! ```
//!
//! # The drawer does not own `is_open`
//!
//! Like every other aura widget, [`Drawer`] is controlled: the
//! application owns whether the panel is open and passes it in, and the
//! widget reports intent through [`Drawer::on_open`] and
//! [`Drawer::on_dismiss`]. Only the *animation* between those two states
//! lives in widget state, because it is presentation rather than
//! application data.
//!
//! # Why a `stack`, and not an overlay
//!
//! The same reason [`dialog`](crate::dialog) gives: `overlay::Overlay`
//! has no `semantics` hook, so anything drawn that way is invisible to
//! the accessibility engine. A navigation drawer is exactly the thing a
//! screen-reader user needs to reach, so it is an ordinary widget layered
//! over the page with [`stack`](crate::stack).
//!
//! # Focus trapping is the caller's to complete
//!
//! While open, a modal drawer owns focus: Tab must not wander into the
//! inert page behind it. That is `aura_accessibility::focus::Scope`'s
//! job and it lives in application state, because focus outlives any
//! single view. This widget reports the drawer's open state and offers
//! `DISMISS`, which is what a caller pushes a `Scope` *for*; see
//! `docs/accessibility/contract.md`.

use crate::core::animation::Animation;
use crate::core::border::Border;
use crate::core::keyboard;
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::time::Instant;
use crate::core::touch;
use crate::core::widget::Operation;
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::visit;
use crate::core::{
    Background, Color, Element, Event, Layout, Length, Padding, Point, Rectangle, Shadow, Shell,
    Size, Theme, Vector, Widget,
};

/// The default width of a drawer panel.
const DEFAULT_WIDTH: f32 = 300.0;

/// How wide the strip along the screen edge is that begins an
/// edge-swipe. 20 logical pixels is the usual platform figure: wide
/// enough to hit without looking, narrow enough not to steal drags that
/// belong to the content underneath.
const DEFAULT_EDGE_WIDTH: f32 = 20.0;

/// How far the panel must be dragged, as a fraction of its width, before
/// releasing commits to the new state. Used only when the drag ends too
/// slowly for [`FLING_VELOCITY`] to decide.
const COMMIT_FRACTION: f32 = 0.5;

/// Speed, in logical pixels per second, past which a drag is a fling: the
/// *direction* decides the outcome and how far the panel actually
/// travelled stops mattering. This is what makes a short, fast flick
/// close the drawer even though it never reached the halfway point.
const FLING_VELOCITY: f32 = 400.0;

/// Which edge a [`Drawer`] slides in from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Side {
    /// The leading edge -- the left in a left-to-right layout.
    #[default]
    Start,
    /// The trailing edge -- the right in a left-to-right layout.
    End,
}

/// An in-progress drag of the panel.
#[derive(Debug, Clone, Copy)]
struct Drag {
    /// Where the pointer went down, in the drawer's own coordinates.
    origin: Point,
    /// The panel's visible fraction when the drag started.
    start_progress: f32,
    /// The most recent pointer position.
    current: Point,
    /// The most recent pointer position that produced a velocity sample.
    last: Point,
    /// When [`Drag::last`] was recorded.
    last_at: Instant,
    /// Signed pointer speed along the drag axis, in pixels per second.
    velocity: f32,
    /// Whether this drag began on the edge strip while the drawer was
    /// closed, and is therefore trying to *open* it.
    from_edge: bool,
}

/// The widget state of a [`Drawer`].
#[derive(Debug)]
pub struct State {
    /// Drives the slide between closed and open.
    animation: Animation<bool>,
    /// The in-progress drag, if the user is currently moving the panel.
    drag: Option<Drag>,
    /// The most recent frame time, kept so `draw` can sample the
    /// animation at the same instant `update` last saw.
    now: Instant,
}

impl Default for State {
    fn default() -> Self {
        Self {
            animation: Animation::new(false).quick(),
            drag: None,
            now: Instant::now(),
        }
    }
}

impl State {
    /// How much of the panel is visible, from `0.0` (fully off screen) to
    /// `1.0` (fully in view).
    ///
    /// A drag in progress wins over the animation: while the user's
    /// finger is down the panel tracks it exactly, which is what makes
    /// the gesture feel attached rather than merely triggered.
    fn progress(&self, width: f32, side: Side) -> f32 {
        match &self.drag {
            Some(drag) => {
                let travelled = match side {
                    Side::Start => drag.current.x - drag.origin.x,
                    Side::End => drag.origin.x - drag.current.x,
                };

                (drag.start_progress + travelled / width.max(1.0)).clamp(0.0, 1.0)
            }
            None => self.animation.interpolate(0.0, 1.0, self.now),
        }
    }
}

/// A navigation panel that slides in from the edge of the screen.
#[allow(missing_debug_implementations)]
pub struct Drawer<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    content: Element<'a, Message, Theme, Renderer>,
    is_open: bool,
    side: Side,
    width: f32,
    padding: Padding,
    on_open: Option<Message>,
    on_dismiss: Option<Message>,
    dismiss_on_scrim: bool,
    edge_swipe_width: f32,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> Drawer<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// Creates a drawer around `content`, open or closed as given.
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>, is_open: bool) -> Self {
        Self {
            content: content.into(),
            is_open,
            side: Side::default(),
            width: DEFAULT_WIDTH,
            padding: Padding::ZERO,
            on_open: None,
            on_dismiss: None,
            dismiss_on_scrim: true,
            edge_swipe_width: DEFAULT_EDGE_WIDTH,
            class: Theme::default(),
        }
    }

    /// Sets which edge the drawer slides in from.
    pub fn side(mut self, side: Side) -> Self {
        self.side = side;
        self
    }

    /// Sets the width of the panel.
    pub fn width(mut self, width: impl Into<crate::core::Pixels>) -> Self {
        self.width = width.into().0.max(1.0);
        self
    }

    /// Sets the interior padding of the panel.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Produces `on_open` when an edge swipe asks for the drawer.
    ///
    /// Without this, the drawer can still be opened by the application
    /// setting `is_open`, but the edge-swipe gesture does nothing --
    /// there would be nobody to tell.
    pub fn on_open(mut self, on_open: Message) -> Self {
        self.on_open = Some(on_open);
        self
    }

    /// Produces `on_dismiss` on Escape, a scrim tap, or a drag that
    /// closes the panel.
    ///
    /// A drawer without this cannot be dismissed by the user at all,
    /// which is appropriate only for a permanently visible navigation
    /// rail.
    pub fn on_dismiss(mut self, on_dismiss: Message) -> Self {
        self.on_dismiss = Some(on_dismiss);
        self
    }

    /// Sets whether tapping the scrim dismisses the drawer.
    pub fn dismiss_on_scrim(mut self, dismiss: bool) -> Self {
        self.dismiss_on_scrim = dismiss;
        self
    }

    /// Sets how wide the edge-swipe strip is. Zero disables edge swiping.
    pub fn edge_swipe_width(mut self, width: impl Into<crate::core::Pixels>) -> Self {
        self.edge_swipe_width = width.into().0.max(0.0);
        self
    }

    /// Sets the style of the drawer.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Whether this drawer is modal, and therefore needs a focus trap
    /// while it is open.
    pub const fn is_modal(&self) -> bool {
        self.is_open
    }

    /// Whether the user can dismiss this drawer.
    pub const fn is_dismissible(&self) -> bool {
        self.on_dismiss.is_some()
    }

    /// Where the panel sits, given how much of it is showing.
    fn surface_bounds(&self, scrim: Size, progress: f32) -> Rectangle {
        let width = self.width.min(scrim.width);

        let x = match self.side {
            // At `progress` 0 the panel is exactly its own width off
            // screen; at 1 its edge meets the screen edge.
            Side::Start => (progress - 1.0) * width,
            Side::End => scrim.width - progress * width,
        };

        Rectangle {
            x,
            y: 0.0,
            width,
            height: scrim.height,
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Drawer<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        // The animation starts *settled* at whatever the caller passed,
        // so a view that is built with the drawer already open does not
        // play an opening animation nobody asked for.
        tree::State::new(State {
            animation: Animation::new(self.is_open).quick(),
            ..State::default()
        })
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
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
        let scrim = limits.resolve(Length::Fill, Length::Fill, Size::ZERO);

        let state = tree.state.downcast_ref::<State>();
        let progress = state.progress(self.width, self.side);
        let surface = self.surface_bounds(scrim, progress);

        let content_limits = layout::Limits::new(
            Size::ZERO,
            Size::new(surface.width, surface.height),
        )
        .shrink(self.padding);

        let content = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, &content_limits)
            .move_to(Point::new(
                surface.x + self.padding.left,
                surface.y + self.padding.top,
            ));

        // Two children, matching `dialog`: the panel rectangle, then the
        // content. The panel node carries no widget -- it exists so
        // `draw` and hit-testing know where the drawer itself is, as
        // distinct from the scrim.
        layout::Node::with_children(
            scrim,
            vec![
                layout::Node::new(Size::new(surface.width, surface.height))
                    .move_to(Point::new(surface.x, surface.y)),
                content,
            ],
        )
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Node, Role, State as SemanticState};

        let state = tree.state.downcast_ref::<State>();
        let progress = state.progress(self.width, self.side);

        // A fully closed drawer is not merely off screen, it is absent:
        // reporting it as a visible window would put a navigation panel
        // in the accessibility tree that the user cannot see or reach.
        if progress <= 0.0 {
            return None;
        }

        let bounds = layout
            .children()
            .next()
            .map(|surface| surface.bounds())
            .unwrap_or_else(|| layout.bounds());

        let mut flags = SemanticState::empty();

        if self.is_open {
            flags |= SemanticState::EXPANDED;
        }

        Some(
            Node::new(tree.id().clone(), bounds, Role::Window)
                .with_state(flags)
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
        let bounds = layout.bounds();
        let mut children = layout.children();
        let surface = children.next().map(|node| node.bounds());
        let content_layout = children.next();

        {
            let state = tree.state.downcast_mut::<State>();

            // Catch up with the caller: `is_open` is application state,
            // so a change to it is what starts the slide.
            if state.animation.value() != self.is_open {
                state.animation.go_mut(self.is_open, state.now);
                shell.request_redraw();
            }

            if let Event::Window(crate::core::window::Event::RedrawRequested(now)) = event {
                state.now = *now;

                if state.animation.is_animating(*now) {
                    shell.request_redraw();
                }
            }
        }

        let progress = tree
            .state
            .downcast_ref::<State>()
            .progress(self.width, self.side);

        // The content sees events first, but only while there is a panel
        // to interact with: a closed drawer must not swallow clicks
        // meant for the page it is stacked over.
        if progress > 0.0 {
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
        }

        match event {
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. })
                if progress > 0.0
                    && *key == keyboard::Key::Named(keyboard::key::Named::Escape) =>
            {
                if let Some(on_dismiss) = self.on_dismiss.clone() {
                    shell.publish(on_dismiss);
                    shell.capture_event();
                }
            }

            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                let Some(position) = cursor.position_over(bounds) else {
                    return;
                };

                let on_surface = surface.is_some_and(|surface| surface.contains(position));

                if on_surface {
                    self.begin_drag(tree, position, progress, false, shell);
                } else if progress > 0.0 {
                    // A press on the scrim: either dismiss, or swallow it
                    // so the inert page behind stays inert.
                    if self.dismiss_on_scrim && let Some(on_dismiss) = self.on_dismiss.clone() {
                        shell.publish(on_dismiss);
                    }

                    shell.capture_event();
                } else if self.is_on_edge_strip(position, bounds) && self.on_open.is_some() {
                    self.begin_drag(tree, position, progress, true, shell);
                }
            }

            Event::Mouse(mouse::Event::CursorMoved { position })
            | Event::Touch(touch::Event::FingerMoved { position, .. }) => {
                let state = tree.state.downcast_mut::<State>();

                if let Some(drag) = &mut state.drag {
                    drag.current = *position;

                    // Velocity is sampled between moves rather than over
                    // the whole gesture, so a drag that stalls and then
                    // flicks is read as a flick.
                    let elapsed = state.now.saturating_duration_since(drag.last_at).as_secs_f32();

                    if elapsed > 0.0 {
                        let travelled = match self.side {
                            Side::Start => position.x - drag.last.x,
                            Side::End => drag.last.x - position.x,
                        };

                        drag.velocity = travelled / elapsed;
                        drag.last = *position;
                        drag.last_at = state.now;
                    }

                    shell.capture_event();
                    shell.request_redraw();
                }
            }

            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. })
            | Event::Touch(touch::Event::FingerLost { .. }) => {
                self.end_drag(tree, shell);
            }

            _ => {
                // Anything else that lands on an open drawer stops here:
                // the page behind is inert, not merely covered.
                if progress > 0.0
                    && matches!(event, Event::Mouse(_) | Event::Touch(_))
                    && cursor.is_over(bounds)
                {
                    shell.capture_event();
                }
            }
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

        if state.progress(self.width, self.side) <= 0.0 {
            return mouse::Interaction::None;
        }

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
        let state = tree.state.downcast_ref::<State>();
        let progress = state.progress(self.width, self.side);

        // Nothing to draw, and nothing to dim.
        if progress <= 0.0 {
            return;
        }

        let appearance = theme.style(&self.class);
        let mut children = layout.children();

        // The scrim fades in with the panel, so a half-open drawer dims
        // the page by half. Anything else makes the gesture feel like it
        // is dragging two unrelated things.
        if let Background::Color(color) = appearance.scrim {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds(),
                    ..renderer::Quad::default()
                },
                Background::Color(Color {
                    a: color.a * progress,
                    ..color
                }),
            );
        }

        if let Some(surface) = children.next() {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: surface.bounds(),
                    border: appearance.border,
                    shadow: appearance.shadow,
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

impl<Message, Theme, Renderer> Drawer<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// Whether `position` is within the edge strip that begins an
    /// edge-swipe.
    fn is_on_edge_strip(&self, position: Point, bounds: Rectangle) -> bool {
        if self.edge_swipe_width <= 0.0 {
            return false;
        }

        match self.side {
            Side::Start => position.x <= bounds.x + self.edge_swipe_width,
            Side::End => position.x >= bounds.x + bounds.width - self.edge_swipe_width,
        }
    }

    fn begin_drag(
        &self,
        tree: &mut Tree,
        position: Point,
        progress: f32,
        from_edge: bool,
        shell: &mut Shell<'_, Message>,
    ) {
        let state = tree.state.downcast_mut::<State>();

        state.drag = Some(Drag {
            origin: position,
            start_progress: progress,
            current: position,
            last: position,
            last_at: state.now,
            velocity: 0.0,
            from_edge,
        });

        shell.capture_event();
    }

    /// Ends a drag and decides what it meant.
    ///
    /// A fast flick is decided by its direction; a slow drag by how far
    /// it got. Either way the widget only *reports* the outcome -- the
    /// application owns `is_open`, so the panel does not move until the
    /// message comes back as a new `is_open`.
    fn end_drag(&self, tree: &mut Tree, shell: &mut Shell<'_, Message>) {
        let state = tree.state.downcast_mut::<State>();

        let Some(drag) = state.drag.take() else {
            return;
        };

        let progress = {
            let travelled = match self.side {
                Side::Start => drag.current.x - drag.origin.x,
                Side::End => drag.origin.x - drag.current.x,
            };

            (drag.start_progress + travelled / self.width.max(1.0)).clamp(0.0, 1.0)
        };

        let should_open = if drag.velocity.abs() >= FLING_VELOCITY {
            drag.velocity > 0.0
        } else {
            progress >= COMMIT_FRACTION
        };

        // A drag that began on the edge strip can only ever open; a drag
        // on the panel itself can only ever close. Letting either do both
        // would mean a stray movement during a tap could flip the drawer.
        let message = if drag.from_edge {
            should_open.then(|| self.on_open.clone()).flatten()
        } else {
            (!should_open).then(|| self.on_dismiss.clone()).flatten()
        };

        if let Some(message) = message {
            shell.publish(message);
        }

        // The panel is mid-slide wherever the finger left it, so the
        // animation has to be told where it is starting from.
        state.animation.go_mut(should_open, state.now);

        shell.capture_event();
        shell.request_redraw();
    }
}

impl<'a, Message, Theme, Renderer> From<Drawer<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a + Catalog,
    Renderer: 'a + crate::core::Renderer,
{
    fn from(drawer: Drawer<'a, Message, Theme, Renderer>) -> Self {
        Self::new(drawer)
    }
}

/// The appearance of a [`Drawer`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The scrim covering the page behind, at full opacity. The drawer
    /// fades it in proportionally as the panel slides.
    pub scrim: Background,
    /// The panel's fill.
    pub background: Background,
    /// The colour the panel's content inherits.
    pub text_color: Color,
    /// The border around the panel.
    pub border: Border,
    /// The shadow the panel casts over the page.
    pub shadow: Shadow,
}

/// The theme catalog of a [`Drawer`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for a [`Drawer`].
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

/// The default styling of a [`Drawer`] for aura's own theme.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        scrim: Background::Color(Color {
            a: 0.4,
            ..Color::BLACK
        }),
        background: Background::Color(palette.background.base.color),
        text_color: palette.background.base.text,
        // The panel meets three screen edges; rounding it would leave
        // gaps against them.
        border: Border::default(),
        shadow: Shadow {
            color: Color {
                a: 0.25,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 0.0),
            blur_radius: 16.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_at(progress_source: Option<Drag>) -> State {
        State {
            animation: Animation::new(false).quick(),
            drag: progress_source,
            now: Instant::now(),
        }
    }

    fn drag_from(origin_x: f32, current_x: f32, start_progress: f32) -> Drag {
        let now = Instant::now();

        Drag {
            origin: Point::new(origin_x, 0.0),
            start_progress,
            current: Point::new(current_x, 0.0),
            last: Point::new(current_x, 0.0),
            last_at: now,
            velocity: 0.0,
            from_edge: false,
        }
    }

    #[test]
    fn a_closed_drawer_sits_exactly_one_width_off_screen() {
        let drawer: Drawer<'_, ()> = Drawer::new(crate::text::Text::new("nav"), false);
        let scrim = Size::new(1000.0, 800.0);

        let start = drawer.surface_bounds(scrim, 0.0);
        assert_eq!(start.x, -DEFAULT_WIDTH);

        let end = drawer.side(Side::End).surface_bounds(scrim, 0.0);
        assert_eq!(end.x, scrim.width);
    }

    #[test]
    fn an_open_drawer_meets_its_edge() {
        let drawer: Drawer<'_, ()> = Drawer::new(crate::text::Text::new("nav"), true);
        let scrim = Size::new(1000.0, 800.0);

        assert_eq!(drawer.surface_bounds(scrim, 1.0).x, 0.0);
        assert_eq!(
            drawer.side(Side::End).surface_bounds(scrim, 1.0).x,
            scrim.width - DEFAULT_WIDTH
        );
    }

    #[test]
    fn dragging_tracks_the_pointer_and_clamps_at_both_ends() {
        // Half a panel-width of travel from closed is half open.
        let state = state_at(Some(drag_from(0.0, DEFAULT_WIDTH / 2.0, 0.0)));
        assert!((state.progress(DEFAULT_WIDTH, Side::Start) - 0.5).abs() < f32::EPSILON);

        // Dragging further than the panel is wide does not over-open it.
        let state = state_at(Some(drag_from(0.0, DEFAULT_WIDTH * 3.0, 0.0)));
        assert_eq!(state.progress(DEFAULT_WIDTH, Side::Start), 1.0);

        // Nor does dragging backwards past closed.
        let state = state_at(Some(drag_from(0.0, -DEFAULT_WIDTH * 3.0, 0.0)));
        assert_eq!(state.progress(DEFAULT_WIDTH, Side::Start), 0.0);
    }

    #[test]
    fn an_end_side_drawer_opens_by_dragging_the_other_way() {
        // Leftward travel opens a right-hand drawer.
        let state = state_at(Some(drag_from(1000.0, 1000.0 - DEFAULT_WIDTH / 2.0, 0.0)));

        assert!((state.progress(DEFAULT_WIDTH, Side::End) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn the_edge_strip_only_covers_its_own_edge() {
        let drawer: Drawer<'_, ()> = Drawer::new(crate::text::Text::new("nav"), false);
        let bounds = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 800.0,
        };

        assert!(drawer.is_on_edge_strip(Point::new(4.0, 100.0), bounds));
        assert!(!drawer.is_on_edge_strip(Point::new(500.0, 100.0), bounds));
        assert!(!drawer.is_on_edge_strip(Point::new(996.0, 100.0), bounds));

        let drawer = drawer.side(Side::End);
        assert!(drawer.is_on_edge_strip(Point::new(996.0, 100.0), bounds));
        assert!(!drawer.is_on_edge_strip(Point::new(4.0, 100.0), bounds));
    }

    #[test]
    fn edge_swiping_can_be_turned_off() {
        let drawer: Drawer<'_, ()> =
            Drawer::new(crate::text::Text::new("nav"), false).edge_swipe_width(0.0);

        let bounds = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 800.0,
        };

        assert!(!drawer.is_on_edge_strip(Point::new(0.0, 100.0), bounds));
    }
}
