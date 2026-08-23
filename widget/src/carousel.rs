//! Horizontally paged content with a swipe gesture and page dots.
//!
//! # Example
//! ```no_run
//! # mod aura { pub mod widget { pub use aura_widget::*; } pub use aura_widget::Renderer; pub use aura_widget::core::*; }
//! # pub type Element<'a, Message> = aura_widget::core::Element<'a, Message, aura_widget::Theme, aura_widget::Renderer>;
//! use aura::widget::{carousel, text};
//!
//! #[derive(Clone)]
//! enum Message {
//!     PageChanged(usize),
//! }
//!
//! # struct State { page: usize }
//! fn view(state: &State) -> Element<'_, Message> {
//!     carousel([text("one").into(), text("two").into()])
//!         .page(state.page)
//!         .on_page_change(Message::PageChanged)
//!         .into()
//! }
//! ```
//!
//! # Only the visible pages are laid out
//!
//! A carousel lays out the current page and, while a swipe is in
//! progress, the one being dragged towards. The rest are not measured,
//! drawn, or visited. A ten-page carousel that laid out all ten would
//! pay for nine screens nobody is looking at, on every frame.
//!
//! # A swipe commits by distance or by speed
//!
//! Releasing past a third of the width turns the page; so does a fast
//! flick that never got that far. Either alone is wrong -- a slow, long
//! drag should commit, and so should a short, quick one.

use crate::core::animation::Animation;
use crate::core::border::Border;
use crate::core::layout;
use crate::core::mouse;
use crate::core::renderer;
use crate::core::time::Instant;
use crate::core::touch;
use crate::core::widget::Operation;
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::visit;
use crate::core::{
    Background, Element, Event, Layout, Length, Point, Rectangle, Shell, Size, Theme, Widget,
};

/// The share of the carousel's width a drag must cross to turn the page
/// on distance alone.
const COMMIT_FRACTION: f32 = 1.0 / 3.0;

/// Speed, in logical pixels per second, past which a drag is a flick and
/// its direction alone turns the page.
const FLING_VELOCITY: f32 = 400.0;

/// The diameter of a page dot.
const DOT_SIZE: f32 = 8.0;

/// The gap between dots.
const DOT_GAP: f32 = 8.0;

/// How far above the bottom edge the dots sit.
const DOT_MARGIN: f32 = 16.0;

/// An in-progress swipe.
#[derive(Debug, Clone, Copy)]
struct Swipe {
    /// Where the pointer went down.
    origin: Point,
    /// The most recent pointer position.
    current: Point,
    /// The most recent position that produced a velocity sample.
    last: Point,
    /// When [`Swipe::last`] was recorded.
    last_at: Instant,
    /// Horizontal pointer speed in pixels per second. Negative is
    /// leftward, which advances the carousel.
    velocity: f32,
}

impl Swipe {
    /// How far the content has been dragged. Negative means the next
    /// page is being pulled in from the right.
    fn offset(&self) -> f32 {
        self.current.x - self.origin.x
    }
}

/// The widget state of a [`Carousel`].
#[derive(Debug)]
pub struct State {
    /// The in-progress swipe, if any.
    swipe: Option<Swipe>,
    /// Settles the content back after a release.
    settle: Animation<f32>,
    /// The most recent frame time.
    now: Instant,
}

impl Default for State {
    fn default() -> Self {
        Self {
            swipe: None,
            settle: Animation::new(0.0).quick(),
            now: Instant::now(),
        }
    }
}

/// Horizontally paged content with a swipe gesture and page dots.
#[allow(missing_debug_implementations)]
pub struct Carousel<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    pages: Vec<Element<'a, Message, Theme, Renderer>>,
    page: usize,
    show_dots: bool,
    on_page_change: Option<Box<dyn Fn(usize) -> Message + 'a>>,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> Carousel<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// Creates a carousel over `pages`.
    pub fn new(pages: impl IntoIterator<Item = Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            pages: pages.into_iter().collect(),
            page: 0,
            show_dots: true,
            on_page_change: None,
            class: Theme::default(),
        }
    }

    /// Sets which page is showing.
    pub fn page(mut self, page: usize) -> Self {
        self.page = page;
        self
    }

    /// Sets whether the page dots are drawn.
    pub fn show_dots(mut self, show_dots: bool) -> Self {
        self.show_dots = show_dots;
        self
    }

    /// Produces a message naming the page a swipe settled on.
    ///
    /// Without it the carousel cannot be swiped at all -- the gesture
    /// would have nowhere to report to, and a page that sprang back
    /// every time would read as broken rather than as fixed.
    pub fn on_page_change(mut self, on_page_change: impl Fn(usize) -> Message + 'a) -> Self {
        self.on_page_change = Some(Box::new(on_page_change));
        self
    }

    /// Sets the style of the page dots.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// The page actually showing, with an out-of-range caller index
    /// clamped to the last page.
    ///
    /// Clamping rather than showing nothing: a carousel with no visible
    /// page is a blank rectangle, which is worse than showing the wrong
    /// one.
    fn current(&self) -> usize {
        if self.pages.is_empty() {
            return 0;
        }

        self.page.min(self.pages.len() - 1)
    }

    /// The page a drag of `offset` is heading towards, if any.
    fn neighbour(&self, offset: f32) -> Option<usize> {
        let current = self.current();

        if offset < 0.0 {
            (current + 1 < self.pages.len()).then_some(current + 1)
        } else if offset > 0.0 {
            current.checked_sub(1)
        } else {
            None
        }
    }

    /// Where a settled release should land.
    ///
    /// A fast flick is decided by its direction alone; a slower drag by
    /// how far it got. Either rule on its own is wrong: a long, slow
    /// drag should commit, and so should a short, quick one.
    fn settled_page(&self, offset: f32, velocity: f32, width: f32) -> usize {
        let current = self.current();

        // `-1` steps back, `1` steps forward, `0` springs back.
        let direction = if velocity.abs() >= FLING_VELOCITY {
            if velocity < 0.0 { 1 } else { -1 }
        } else if width > 0.0 && offset.abs() >= width * COMMIT_FRACTION {
            // Guarded on width: with a zero-width carousel every drag
            // would clear a threshold of zero and turn the page.
            if offset < 0.0 { 1 } else { -1 }
        } else {
            0
        };

        match direction {
            1 if current + 1 < self.pages.len() => current + 1,
            -1 => current.saturating_sub(1),
            _ => current,
        }
    }

    /// The dots' bounds, when they are drawn.
    fn dots_bounds(&self, bounds: Rectangle) -> Option<Rectangle> {
        if !self.show_dots || self.pages.len() < 2 {
            return None;
        }

        let count = self.pages.len() as f32;
        let width = count * DOT_SIZE + (count - 1.0) * DOT_GAP;

        Some(Rectangle {
            x: bounds.x + (bounds.width - width) / 2.0,
            y: bounds.y + bounds.height - DOT_MARGIN - DOT_SIZE,
            width,
            height: DOT_SIZE,
        })
    }
}

impl<Message, Theme, Renderer> Carousel<'_, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// The content's horizontal offset right now.
    fn offset(&self, state: &State) -> f32 {
        match &state.swipe {
            Some(swipe) => swipe.offset(),
            None => state.settle.interpolate(state.now),
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Carousel<'_, Message, Theme, Renderer>
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
        // Every page keeps its tree, whether or not it is on screen, so
        // swiping back does not reset the state a page was holding.
        tree.diff_children(&mut self.pages);
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
        let bounds = limits.resolve(Length::Fill, Length::Fill, Size::ZERO);

        if self.pages.is_empty() {
            return layout::Node::new(bounds);
        }

        let state = tree.state.downcast_ref::<State>();
        let offset = self.offset(state);

        let current = self.current();
        let neighbour = self.neighbour(offset);

        let page_limits = layout::Limits::new(Size::ZERO, bounds);

        let mut children = Vec::with_capacity(2);

        children.push(
            self.pages[current]
                .as_widget_mut()
                .layout(&mut tree.children[current], renderer, &page_limits)
                .move_to(Point::new(offset, 0.0)),
        );

        // The incoming page is placed one full width away in the
        // direction of travel, so it slides in as the current one leaves.
        if let Some(neighbour) = neighbour {
            let x = if neighbour > current {
                offset + bounds.width
            } else {
                offset - bounds.width
            };

            children.push(
                self.pages[neighbour]
                    .as_widget_mut()
                    .layout(&mut tree.children[neighbour], renderer, &page_limits)
                    .move_to(Point::new(x, 0.0)),
            );
        }

        layout::Node::with_children(bounds, children)
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Node, Role, State as SemanticState};

        Some(
            Node::new(tree.id().clone(), layout.bounds(), Role::Group)
                .with_state(SemanticState::empty())
                .with_actions(Actions::empty()),
        )
    }

    fn visit_children(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        if self.pages.is_empty() {
            return;
        }

        let current = self.current();

        if let Some(page_layout) = layout.children().next() {
            visit(&mut visit::Widgets::new(
                self.pages[current].as_widget_mut(),
                &mut tree.children[current],
                page_layout,
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
            if self.pages.is_empty() {
                return;
            }

            let current = self.current();

            if let Some(page_layout) = layout.children().next() {
                self.pages[current].as_widget_mut().operate(
                    &mut tree.children[current],
                    page_layout,
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

        if self.pages.is_empty() {
            return;
        }

        let current = self.current();

        // The page's own content sees events first, so a button on a
        // slide wins its click before the swipe gesture considers it.
        if let Some(page_layout) = layout.children().next() {
            self.pages[current].as_widget_mut().update(
                &mut tree.children[current],
                event,
                page_layout,
                cursor,
                renderer,
                shell,
                viewport,
            );

            if shell.is_event_captured() {
                return;
            }
        }

        let Some(on_page_change) = &self.on_page_change else {
            return;
        };

        let bounds = layout.bounds();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                let Some(position) = cursor.position_over(bounds) else {
                    return;
                };

                let state = tree.state.downcast_mut::<State>();

                state.swipe = Some(Swipe {
                    origin: position,
                    current: position,
                    last: position,
                    last_at: state.now,
                    velocity: 0.0,
                });
            }

            Event::Mouse(mouse::Event::CursorMoved { position })
            | Event::Touch(touch::Event::FingerMoved { position, .. }) => {
                let state = tree.state.downcast_mut::<State>();

                let Some(swipe) = &mut state.swipe else {
                    return;
                };

                swipe.current = *position;

                let elapsed = state
                    .now
                    .saturating_duration_since(swipe.last_at)
                    .as_secs_f32();

                if elapsed > 0.0 {
                    swipe.velocity = (position.x - swipe.last.x) / elapsed;
                    swipe.last = *position;
                    swipe.last_at = state.now;
                }

                shell.capture_event();
                shell.request_redraw();
            }

            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. })
            | Event::Touch(touch::Event::FingerLost { .. }) => {
                let state = tree.state.downcast_mut::<State>();

                let Some(swipe) = state.swipe.take() else {
                    return;
                };

                let settled = self.settled_page(swipe.offset(), swipe.velocity, bounds.width);

                if settled != current {
                    shell.publish(on_page_change(settled));
                }

                // Spring back to rest from wherever the finger left it;
                // the new page arrives as the caller's `page` changes.
                state.settle = Animation::new(swipe.offset()).quick();
                state.settle.go_mut(0.0, state.now);

                shell.capture_event();
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
        if self.pages.is_empty() {
            return mouse::Interaction::None;
        }

        let current = self.current();

        layout
            .children()
            .next()
            .map(|page_layout| {
                self.pages[current].as_widget().mouse_interaction(
                    &tree.children[current],
                    page_layout,
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
        if self.pages.is_empty() {
            return;
        }

        let bounds = layout.bounds();
        let state = tree.state.downcast_ref::<State>();
        let offset = self.offset(state);

        let current = self.current();
        let neighbour = self.neighbour(offset);

        let mut children = layout.children();

        if let Some(page_layout) = children.next() {
            self.pages[current].as_widget().draw(
                &tree.children[current],
                renderer,
                theme,
                style,
                page_layout,
                cursor,
                viewport,
            );
        }

        if let (Some(neighbour), Some(page_layout)) = (neighbour, children.next()) {
            self.pages[neighbour].as_widget().draw(
                &tree.children[neighbour],
                renderer,
                theme,
                style,
                page_layout,
                cursor,
                viewport,
            );
        }

        let Some(dots) = self.dots_bounds(bounds) else {
            return;
        };

        let appearance = theme.style(&self.class);

        for index in 0..self.pages.len() {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: dots.x + index as f32 * (DOT_SIZE + DOT_GAP),
                        y: dots.y,
                        width: DOT_SIZE,
                        height: DOT_SIZE,
                    },
                    border: Border {
                        radius: (DOT_SIZE / 2.0).into(),
                        ..Border::default()
                    },
                    ..renderer::Quad::default()
                },
                if index == current {
                    appearance.active_dot
                } else {
                    appearance.dot
                },
            );
        }
    }
}

impl<'a, Message, Theme, Renderer> From<Carousel<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a + Catalog,
    Renderer: 'a + crate::core::Renderer,
{
    fn from(carousel: Carousel<'a, Message, Theme, Renderer>) -> Self {
        Self::new(carousel)
    }
}

/// The appearance of a [`Carousel`]'s page dots.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// An inactive page's dot.
    pub dot: Background,
    /// The current page's dot.
    pub active_dot: Background,
}

/// The theme catalog of a [`Carousel`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for a [`Carousel`].
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

/// The default styling of a [`Carousel`] for aura's own theme.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        dot: Background::Color(crate::core::Color {
            a: 0.3,
            ..palette.background.base.text
        }),
        active_dot: Background::Color(palette.primary.base.color),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn carousel<'a>(count: usize) -> Carousel<'a, ()> {
        Carousel::new(
            (0..count).map(|index| crate::text::Text::new(format!("page {index}")).into()),
        )
    }

    const WIDTH: f32 = 300.0;

    const BOUNDS: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: WIDTH,
        height: 400.0,
    };

    #[test]
    fn an_out_of_range_page_clamps_to_the_last_one() {
        assert_eq!(carousel(3).page(99).current(), 2);
        assert_eq!(carousel(3).page(1).current(), 1);
    }

    #[test]
    fn an_empty_carousel_does_not_index_out_of_bounds() {
        assert_eq!(carousel(0).current(), 0);
        assert_eq!(carousel(0).neighbour(-50.0), None);
        assert!(carousel(0).dots_bounds(BOUNDS).is_none());
    }

    #[test]
    fn dragging_left_pulls_in_the_next_page() {
        assert_eq!(carousel(3).page(0).neighbour(-10.0), Some(1));
    }

    #[test]
    fn dragging_right_pulls_in_the_previous_page() {
        assert_eq!(carousel(3).page(1).neighbour(10.0), Some(0));
    }

    #[test]
    fn there_is_no_neighbour_past_either_end() {
        assert_eq!(carousel(3).page(0).neighbour(10.0), None);
        assert_eq!(carousel(3).page(2).neighbour(-10.0), None);
    }

    #[test]
    fn a_resting_carousel_has_no_neighbour_to_lay_out() {
        assert_eq!(carousel(3).page(1).neighbour(0.0), None);
    }

    #[test]
    fn a_short_slow_drag_springs_back() {
        let carousel = carousel(3).page(1);

        assert_eq!(carousel.settled_page(-10.0, 0.0, WIDTH), 1);
        assert_eq!(carousel.settled_page(10.0, 0.0, WIDTH), 1);
    }

    #[test]
    fn a_long_slow_drag_turns_the_page() {
        let carousel = carousel(3).page(1);

        assert_eq!(carousel.settled_page(-WIDTH / 2.0, 0.0, WIDTH), 2);
        assert_eq!(carousel.settled_page(WIDTH / 2.0, 0.0, WIDTH), 0);
    }

    #[test]
    fn a_short_fast_flick_turns_the_page_anyway() {
        let carousel = carousel(3).page(1);

        assert_eq!(carousel.settled_page(-5.0, -1_000.0, WIDTH), 2);
        assert_eq!(carousel.settled_page(5.0, 1_000.0, WIDTH), 0);
    }

    #[test]
    fn a_flick_past_the_last_page_stays_put() {
        let carousel = carousel(3).page(2);

        assert_eq!(carousel.settled_page(-WIDTH, -1_000.0, WIDTH), 2);
    }

    #[test]
    fn a_flick_before_the_first_page_stays_put() {
        let carousel = carousel(3).page(0);

        assert_eq!(carousel.settled_page(WIDTH, 1_000.0, WIDTH), 0);
    }

    #[test]
    fn the_dots_are_centred_and_sit_above_the_bottom_edge() {
        let dots = carousel(3).dots_bounds(BOUNDS).expect("dots");

        assert_eq!(dots.x + dots.width / 2.0, BOUNDS.x + BOUNDS.width / 2.0);
        assert_eq!(dots.y + dots.height, BOUNDS.y + BOUNDS.height - DOT_MARGIN);
    }

    #[test]
    fn a_single_page_carousel_shows_no_dots() {
        // One dot conveys nothing, and a control that cannot move should
        // not advertise that it can.
        assert!(carousel(1).dots_bounds(BOUNDS).is_none());
    }

    #[test]
    fn dots_can_be_turned_off() {
        assert!(carousel(3).show_dots(false).dots_bounds(BOUNDS).is_none());
    }
}
