//! A placeholder standing in for content that has not arrived.
//!
//! # Example
//! ```no_run
//! # mod aura { pub mod widget { pub use aura_widget::*; } pub use aura_widget::Renderer; pub use aura_widget::core::*; }
//! # pub type Element<'a, Message> = aura_widget::core::Element<'a, Message, aura_widget::Theme, aura_widget::Renderer>;
//! use aura::widget::{column, skeleton};
//!
//! # enum Message {}
//! fn view() -> Element<'static, Message> {
//!     column![
//!         skeleton().width(200).height(16),
//!         skeleton().width(160).height(16),
//!     ]
//!     .spacing(8)
//!     .into()
//! }
//! ```
//!
//! # A skeleton says "loading", not nothing
//!
//! The widget reports [`State::BUSY`](aura_core::semantics::State::BUSY)
//! with an accessible name, so a screen reader announces that content is
//! on its way. A placeholder that is only a grey rectangle tells a
//! sighted user something is happening and tells everyone else nothing.
//!
//! # The shimmer respects reduced motion
//!
//! The sweep is driven by
//! [`MotionPolicy`](aura_motion::reduced_motion::MotionPolicy). Under a
//! reduced-motion preference the skeleton holds a steady fill instead of
//! animating: a looping animation is exactly the kind of movement that
//! preference exists to stop, and a skeleton can loop for a long time.

use crate::core::border::Border;
use crate::core::layout;
use crate::core::mouse;
use crate::core::renderer;
use crate::core::time::{Duration, Instant};
use crate::core::widget::tree::{self, Tree};
use crate::core::{
    Background, Color, Element, Event, Layout, Length, Rectangle, Shell, Size, Theme, Widget,
};

/// How long one sweep of the shimmer takes.
const SHIMMER_PERIOD: Duration = Duration::from_millis(1_200);

/// How wide the moving highlight is, as a fraction of the placeholder.
const HIGHLIGHT_FRACTION: f32 = 0.4;

/// The widget state of a [`Skeleton`].
#[derive(Debug)]
pub struct State {
    /// When this placeholder first appeared, so the sweep is continuous
    /// rather than restarting on every frame.
    started_at: Instant,
    /// The most recent frame time.
    now: Instant,
}

impl Default for State {
    fn default() -> Self {
        let now = Instant::now();

        Self {
            started_at: now,
            now,
        }
    }
}

impl State {
    /// How far through the current sweep the shimmer is, in `[0, 1)`.
    fn phase(&self) -> f32 {
        let elapsed = self.now.saturating_duration_since(self.started_at);
        let period = SHIMMER_PERIOD.as_secs_f32();

        if period <= 0.0 {
            return 0.0;
        }

        (elapsed.as_secs_f32() / period).fract()
    }
}

/// A placeholder standing in for content that has not arrived.
#[allow(missing_debug_implementations)]
pub struct Skeleton<'a, Theme = crate::Theme>
where
    Theme: Catalog,
{
    width: Length,
    height: Length,
    radius: f32,
    animated: bool,
    motion_policy: aura_motion::MotionPolicy,
    label: String,
    class: Theme::Class<'a>,
}

impl<'a, Theme> Skeleton<'a, Theme>
where
    Theme: Catalog,
{
    /// Creates a placeholder.
    pub fn new() -> Self {
        Self {
            width: Length::Fill,
            height: Length::Fixed(16.0),
            radius: 4.0,
            animated: true,
            motion_policy: aura_motion::MotionPolicy::Full,
            label: "Loading".to_owned(),
            class: Theme::default(),
        }
    }

    /// Sets the width.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height.
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the corner radius. Use half the height for a pill, or the
    /// full height for a circle standing in for an avatar.
    pub fn radius(mut self, radius: impl Into<crate::core::Pixels>) -> Self {
        self.radius = radius.into().0.max(0.0);
        self
    }

    /// Sets what is loading, used as the accessible name.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Turns the shimmer off, leaving a steady fill.
    ///
    /// A reduced-motion preference already does this; this is for the
    /// separate case of a placeholder small or numerous enough that the
    /// movement would be noise rather than information.
    pub fn still(mut self) -> Self {
        self.animated = false;
        self
    }

    /// Sets the style of the placeholder.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the motion policy this placeholder honours.
    ///
    /// Defaults to [`MotionPolicy::Full`]. `aura-motion` does not read
    /// the platform preference itself -- there is no
    /// `MotionPolicy::from_environment` -- so an application that has
    /// resolved the user's setting passes it in here. Reading it in the
    /// widget would mean every widget guessing separately at something
    /// the platform layer already knows.
    pub fn motion_policy(mut self, policy: aura_motion::MotionPolicy) -> Self {
        self.motion_policy = policy;
        self
    }

    /// Whether the shimmer should actually run.
    ///
    /// The sweep is spatial movement, so it is exactly what a reduced
    /// preference asks to be replaced: under [`MotionPolicy::Reduced`],
    /// `Effect::Transform` maps to `Effect::Fade`, and a skeleton has
    /// nothing meaningful to fade between. It holds a steady fill
    /// instead.
    fn is_animated(&self) -> bool {
        use aura_motion::reduced_motion::Effect;

        self.animated && self.motion_policy.effect(Effect::Transform) == Effect::Transform
    }

    /// The highlight's bounds for a given sweep phase.
    ///
    /// The band travels from just off the leading edge to just off the
    /// trailing one, so it enters and leaves rather than appearing in
    /// place.
    fn highlight_bounds(&self, bounds: Rectangle, phase: f32) -> Rectangle {
        let width = bounds.width * HIGHLIGHT_FRACTION;
        let travel = bounds.width + width;

        Rectangle {
            x: bounds.x - width + phase * travel,
            y: bounds.y,
            width,
            height: bounds.height,
        }
    }
}

impl<Theme> Default for Skeleton<'_, Theme>
where
    Theme: Catalog,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Skeleton<'_, Theme>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.resolve(self.width, self.height, Size::ZERO))
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Node, Role, State as SemanticState};

        Some(
            Node::new(tree.id().clone(), layout.bounds(), Role::StaticText)
                .with_name(self.label.as_str())
                .with_state(SemanticState::BUSY)
                .with_actions(Actions::empty()),
        )
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let Event::Window(crate::core::window::Event::RedrawRequested(now)) = event else {
            return;
        };

        let state = tree.state.downcast_mut::<State>();
        state.now = *now;

        // Only a running shimmer asks for another frame. A still
        // placeholder costs nothing per frame, which is what lets a list
        // of a hundred of them stay at zero idle work.
        if self.is_animated() {
            shell.request_redraw();
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let appearance = theme.style(&self.class);
        let bounds = layout.bounds();

        let border = Border {
            radius: self.radius.into(),
            ..appearance.border
        };

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border,
                ..renderer::Quad::default()
            },
            appearance.background,
        );

        if !self.is_animated() {
            return;
        }

        let state = tree.state.downcast_ref::<State>();
        let highlight = self.highlight_bounds(bounds, state.phase());

        // Clipped to the placeholder so the band does not paint over
        // whatever sits beside it as it enters and leaves.
        let Some(visible) = highlight.intersection(&bounds) else {
            return;
        };

        renderer.fill_quad(
            renderer::Quad {
                bounds: visible,
                border,
                ..renderer::Quad::default()
            },
            appearance.highlight,
        );
    }
}

impl<'a, Message, Theme, Renderer> From<Skeleton<'a, Theme>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a + Catalog,
    Renderer: 'a + crate::core::Renderer,
{
    fn from(skeleton: Skeleton<'a, Theme>) -> Self {
        Self::new(skeleton)
    }
}

/// The appearance of a [`Skeleton`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The placeholder's fill.
    pub background: Background,
    /// The band that sweeps across it.
    pub highlight: Background,
    /// The border around the placeholder.
    pub border: Border,
}

/// The theme catalog of a [`Skeleton`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for a [`Skeleton`].
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

/// The default styling of a [`Skeleton`] for aura's own theme.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        background: Background::Color(palette.background.weak.color),
        // A low-contrast sweep on purpose: a placeholder should not pull
        // the eye harder than the content it is standing in for.
        highlight: Background::Color(Color {
            a: 0.35,
            ..palette.background.strong.color
        }),
        border: Border::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skeleton<'a>() -> Skeleton<'a> {
        Skeleton::new()
    }

    const BOUNDS: Rectangle = Rectangle {
        x: 20.0,
        y: 10.0,
        width: 200.0,
        height: 16.0,
    };

    #[test]
    fn the_highlight_enters_from_before_the_leading_edge() {
        let highlight = skeleton().highlight_bounds(BOUNDS, 0.0);

        assert!(
            highlight.x + highlight.width <= BOUNDS.x,
            "at phase 0 the band should be entirely off the leading edge"
        );
    }

    #[test]
    fn the_highlight_leaves_past_the_trailing_edge() {
        let highlight = skeleton().highlight_bounds(BOUNDS, 1.0);

        assert!(
            highlight.x >= BOUNDS.x + BOUNDS.width,
            "at phase 1 the band should be entirely past the trailing edge"
        );
    }

    #[test]
    fn the_highlight_crosses_the_placeholder_in_between() {
        let highlight = skeleton().highlight_bounds(BOUNDS, 0.5);

        assert!(
            highlight.intersection(&BOUNDS).is_some(),
            "the band should be visible halfway through the sweep"
        );
    }

    #[test]
    fn the_sweep_wraps_rather_than_running_away() {
        let mut state = State::default();
        state.now = state.started_at + SHIMMER_PERIOD * 3 + SHIMMER_PERIOD / 4;

        let phase = state.phase();

        assert!(
            (0.0..1.0).contains(&phase),
            "phase should stay in [0, 1), got {phase}"
        );
        assert!((phase - 0.25).abs() < 0.01);
    }

    #[test]
    fn a_fresh_skeleton_starts_at_the_beginning_of_its_sweep() {
        assert_eq!(State::default().phase(), 0.0);
    }

    #[test]
    fn a_still_skeleton_never_animates() {
        // Independent of the environment's motion policy: this is the
        // caller's own choice and does not need to consult it.
        assert!(!skeleton().still().is_animated());
    }
}
