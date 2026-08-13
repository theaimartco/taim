//! Drag a scrollable past its top to reload it.
//!
//! # Example
//! ```no_run
//! # mod aura { pub mod widget { pub use aura_widget::*; } pub use aura_widget::Renderer; pub use aura_widget::core::*; }
//! # pub type Element<'a, Message> = aura_widget::core::Element<'a, Message, aura_widget::Theme, aura_widget::Renderer>;
//! use aura::widget::{pull_to_refresh, scrollable, text};
//!
//! #[derive(Clone)]
//! enum Message {
//!     Refresh,
//! }
//!
//! # struct State { refreshing: bool }
//! fn view(state: &State) -> Element<'_, Message> {
//!     pull_to_refresh(scrollable(text("a long list")), state.refreshing)
//!         .on_refresh(Message::Refresh)
//!         .into()
//! }
//! ```
//!
//! # The gesture only starts at the top
//!
//! A downward drag is only a refresh when the content underneath cannot
//! scroll down any further. Anywhere else it belongs to the scrollable,
//! and this widget must not take it -- a list that reloaded whenever the
//! user scrolled up quickly would be unusable. The widget therefore lets
//! the content see every event first and only claims the drag once the
//! content has declined to move.
//!
//! # Releasing is what refreshes, and only past the threshold
//!
//! A partial pull springs back and produces nothing. Only a release past
//! [`PullToRefresh::threshold`] publishes
//! [`PullToRefresh::on_refresh`], which is what makes an accidental
//! brush against the top edge harmless.
//!
//! # `is_refreshing` is the caller's
//!
//! Like every other controlled widget here, the spinner's visibility is
//! application state. The widget reports that a refresh was asked for;
//! the application decides when it is over. Otherwise a refresh that
//! failed instantly would still spin for as long as the widget felt like
//! it.

use crate::core::animation::Animation;
use crate::core::border::Border;
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
    Background, Element, Event, Layout, Length, Point, Rectangle, Shell, Size, Theme, Vector,
    Widget,
};

/// How far the content must be dragged before releasing refreshes.
const DEFAULT_THRESHOLD: f32 = 64.0;

/// How far the content may be dragged in total. Past the threshold the
/// pull keeps moving but with increasing resistance, so the gesture
/// still responds without the content sliding off the screen.
const MAX_PULL: f32 = 128.0;

/// The diameter of the indicator.
const INDICATOR_SIZE: f32 = 28.0;

/// How far the indicator sits above the content's resting top edge, so
/// it is hidden until a pull begins.
const INDICATOR_INSET: f32 = 40.0;

/// The share of drag distance that still moves the content once the
/// threshold is passed.
const RESISTANCE: f32 = 0.4;

/// An in-progress pull.
#[derive(Debug, Clone, Copy)]
struct Pull {
    /// Where the pointer went down.
    origin: Point,
    /// The most recent pointer position.
    current: Point,
}

impl Pull {
    /// The raw downward distance travelled, before resistance.
    fn travel(&self) -> f32 {
        (self.current.y - self.origin.y).max(0.0)
    }
}

/// The widget state of a [`PullToRefresh`].
#[derive(Debug)]
pub struct State {
    /// The in-progress pull, if the user is dragging.
    pull: Option<Pull>,
    /// Drives the spring back when a pull is released.
    settle: Animation<f32>,
    /// The most recent frame time.
    now: Instant,
}

impl Default for State {
    fn default() -> Self {
        Self {
            pull: None,
            settle: Animation::new(0.0).quick(),
            now: Instant::now(),
        }
    }
}

/// Drag a scrollable past its top to reload it.
#[allow(missing_debug_implementations)]
pub struct PullToRefresh<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    content: Element<'a, Message, Theme, Renderer>,
    is_refreshing: bool,
    threshold: f32,
    on_refresh: Option<Message>,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> PullToRefresh<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// Wraps `content`, showing a spinner while `is_refreshing`.
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        is_refreshing: bool,
    ) -> Self {
        Self {
            content: content.into(),
            is_refreshing,
            threshold: DEFAULT_THRESHOLD,
            on_refresh: None,
            class: Theme::default(),
        }
    }

    /// Produces `on_refresh` when a pull past the threshold is released.
    ///
    /// Without it the gesture is inert: there would be nobody to tell,
    /// and a spinner nobody can stop is worse than no spinner.
    pub fn on_refresh(mut self, on_refresh: Message) -> Self {
        self.on_refresh = Some(on_refresh);
        self
    }

    /// Sets how far the content must be pulled before releasing
    /// refreshes.
    pub fn threshold(mut self, threshold: impl Into<crate::core::Pixels>) -> Self {
        self.threshold = threshold.into().0.max(1.0);
        self
    }

    /// Sets the style of the indicator.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// How far the content is actually displaced, given raw drag travel.
    ///
    /// Linear up to the threshold, then resisted: the gesture keeps
    /// responding so it never feels stuck, but the content cannot be
    /// dragged arbitrarily far down the screen.
    fn displacement(&self, travel: f32) -> f32 {
        if travel <= self.threshold {
            return travel;
        }

        let past = travel - self.threshold;

        (self.threshold + past * RESISTANCE).min(MAX_PULL)
    }

    /// Whether a release at this displacement should refresh.
    fn commits(&self, displacement: f32) -> bool {
        displacement >= self.threshold
    }

    /// Where the indicator sits for a given displacement.
    fn indicator_bounds(&self, bounds: Rectangle, displacement: f32) -> Rectangle {
        Rectangle {
            x: bounds.x + (bounds.width - INDICATOR_SIZE) / 2.0,
            // Starts above the top edge and is carried down by the pull,
            // so it appears to be dragged into view rather than to fade
            // in on the spot.
            y: bounds.y - INDICATOR_INSET + displacement,
            width: INDICATOR_SIZE,
            height: INDICATOR_SIZE,
        }
    }
}

impl<Message, Theme, Renderer> PullToRefresh<'_, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// The content's current vertical offset.
    ///
    /// A live drag wins; otherwise the settle animation runs, and a
    /// refresh in flight holds the content at the threshold so the
    /// spinner has somewhere to sit.
    fn offset(&self, state: &State) -> f32 {
        match &state.pull {
            Some(pull) => self.displacement(pull.travel()),
            None if self.is_refreshing => self.threshold,
            None => state.settle.interpolate(state.now),
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for PullToRefresh<'_, Message, Theme, Renderer>
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
        let state = tree.state.downcast_ref::<State>();
        let offset = self.offset(state);

        let content = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);

        let size = content.size();

        // The content is displaced, not resized: a list that reflowed
        // while being pulled would reshuffle under the user's finger.
        let content = content.move_to(Point::new(0.0, offset));

        layout::Node::with_children(size, vec![content])
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Node, Role, State as SemanticState};

        // Only says anything while a refresh is actually running. The
        // gesture itself is not a control, and the content underneath
        // reports itself.
        if !self.is_refreshing {
            return None;
        }

        Some(
            Node::new(tree.id().clone(), layout.bounds(), Role::StaticText)
                .with_name("Refreshing")
                .with_state(SemanticState::BUSY)
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
        {
            let state = tree.state.downcast_mut::<State>();

            if let Event::Window(crate::core::window::Event::RedrawRequested(now)) = event {
                state.now = *now;

                if state.settle.is_animating(*now) {
                    shell.request_redraw();
                }
            }
        }

        // The content is offered every event first. This is what keeps
        // the gesture from stealing scrolls: a scrollable that can still
        // move will consume the drag, and only when it declines does the
        // pull below get a chance to start.
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

        let Some(on_refresh) = self.on_refresh.clone() else {
            return;
        };

        // A refresh already in flight owns the indicator; starting a
        // second pull would queue a reload nobody asked for.
        if self.is_refreshing {
            return;
        }

        let bounds = layout.bounds();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                let Some(position) = cursor.position_over(bounds) else {
                    return;
                };

                let state = tree.state.downcast_mut::<State>();

                state.pull = Some(Pull {
                    origin: position,
                    current: position,
                });
            }

            Event::Mouse(mouse::Event::CursorMoved { position })
            | Event::Touch(touch::Event::FingerMoved { position, .. }) => {
                let state = tree.state.downcast_mut::<State>();

                let Some(pull) = &mut state.pull else {
                    return;
                };

                pull.current = *position;

                // Only claim the event once the drag is actually downward.
                // An upward or sideways move belongs to whatever is
                // underneath, and a pull of zero must not swallow it.
                if pull.travel() > 0.0 {
                    shell.capture_event();
                    shell.request_redraw();
                }
            }

            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. })
            | Event::Touch(touch::Event::FingerLost { .. }) => {
                let state = tree.state.downcast_mut::<State>();

                let Some(pull) = state.pull.take() else {
                    return;
                };

                let displacement = self.displacement(pull.travel());

                if self.commits(displacement) {
                    shell.publish(on_refresh);
                }

                // Spring back from wherever the finger left it. The
                // caller may hold it open by setting `is_refreshing`,
                // which `offset` honours over this animation.
                state.settle = Animation::new(displacement).quick();
                state.settle.go_mut(0.0, state.now);

                shell.request_redraw();
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

        let state = tree.state.downcast_ref::<State>();
        let offset = self.offset(state);

        if offset <= 0.0 {
            return;
        }

        let appearance = theme.style(&self.class);
        let bounds = layout.bounds();
        let indicator = self.indicator_bounds(bounds, offset);

        // Clipped to the widget, so a pull that has only just begun does
        // not paint the indicator over whatever sits above it.
        let Some(visible) = indicator.intersection(&bounds) else {
            return;
        };

        renderer.fill_quad(
            renderer::Quad {
                bounds: visible,
                border: Border {
                    radius: (INDICATOR_SIZE / 2.0).into(),
                    ..appearance.border
                },
                ..renderer::Quad::default()
            },
            // Armed once the pull would commit, so the user can see the
            // threshold rather than having to learn it.
            if self.is_refreshing || self.commits(offset) {
                appearance.armed
            } else {
                appearance.background
            },
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

impl<'a, Message, Theme, Renderer> From<PullToRefresh<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a + Catalog,
    Renderer: 'a + crate::core::Renderer,
{
    fn from(pull_to_refresh: PullToRefresh<'a, Message, Theme, Renderer>) -> Self {
        Self::new(pull_to_refresh)
    }
}

/// The appearance of a [`PullToRefresh`] indicator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The indicator's fill while the pull is still short of the
    /// threshold.
    pub background: Background,
    /// The indicator's fill once releasing would refresh.
    pub armed: Background,
    /// The border around the indicator.
    pub border: Border,
}

/// The theme catalog of a [`PullToRefresh`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for a [`PullToRefresh`].
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

/// The default styling of a [`PullToRefresh`] for aura's own theme.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        background: Background::Color(palette.background.strong.color),
        armed: Background::Color(palette.primary.base.color),
        border: Border::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn puller<'a>(is_refreshing: bool) -> PullToRefresh<'a, ()> {
        PullToRefresh::new(crate::text::Text::new("list"), is_refreshing)
    }

    const BOUNDS: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 400.0,
        height: 600.0,
    };

    #[test]
    fn a_short_pull_tracks_the_finger_exactly() {
        let puller = puller(false);

        assert_eq!(puller.displacement(0.0), 0.0);
        assert_eq!(puller.displacement(30.0), 30.0);
        assert_eq!(puller.displacement(DEFAULT_THRESHOLD), DEFAULT_THRESHOLD);
    }

    #[test]
    fn a_pull_past_the_threshold_meets_resistance() {
        let puller = puller(false);
        let past = puller.displacement(DEFAULT_THRESHOLD + 100.0);

        assert!(
            past > DEFAULT_THRESHOLD,
            "the gesture must keep responding past the threshold"
        );
        assert!(
            past < DEFAULT_THRESHOLD + 100.0,
            "but not one-to-one, or the content slides off the screen"
        );
    }

    #[test]
    fn a_pull_can_never_exceed_its_maximum() {
        assert_eq!(puller(false).displacement(10_000.0), MAX_PULL);
    }

    #[test]
    fn only_a_pull_past_the_threshold_commits() {
        let puller = puller(false);

        assert!(!puller.commits(DEFAULT_THRESHOLD - 0.01));
        assert!(puller.commits(DEFAULT_THRESHOLD));
        assert!(puller.commits(MAX_PULL));
    }

    #[test]
    fn a_custom_threshold_moves_the_commit_point() {
        let puller = puller(false).threshold(20.0);

        assert!(puller.commits(20.0));
        assert!(!puller.commits(19.0));
        assert_eq!(puller.displacement(20.0), 20.0);
    }

    #[test]
    fn an_upward_drag_produces_no_pull() {
        let pull = Pull {
            origin: Point::new(0.0, 300.0),
            current: Point::new(0.0, 100.0),
        };

        assert_eq!(pull.travel(), 0.0);
        assert!(!puller(false).commits(puller(false).displacement(pull.travel())));
    }

    #[test]
    fn a_refresh_in_flight_holds_the_content_open() {
        let state = State::default();

        assert_eq!(puller(true).offset(&state), DEFAULT_THRESHOLD);
        assert_eq!(puller(false).offset(&state), 0.0);
    }

    #[test]
    fn the_indicator_starts_out_of_sight_and_is_carried_in() {
        let puller = puller(false);

        let at_rest = puller.indicator_bounds(BOUNDS, 0.0);
        assert!(
            at_rest.y + at_rest.height <= BOUNDS.y,
            "the indicator should be hidden until a pull begins"
        );

        let pulled = puller.indicator_bounds(BOUNDS, DEFAULT_THRESHOLD);
        assert!(pulled.y > at_rest.y, "the pull should carry it down");
        assert!(
            pulled.intersection(&BOUNDS).is_some(),
            "and it should be visible once the threshold is reached"
        );
    }

    #[test]
    fn the_indicator_stays_horizontally_centred() {
        let indicator = puller(false).indicator_bounds(BOUNDS, 40.0);

        assert_eq!(
            indicator.x + indicator.width / 2.0,
            BOUNDS.x + BOUNDS.width / 2.0
        );
    }
}
