//! A modal panel that slides up from the bottom of the screen.
//!
//! # Example
//! ```no_run
//! # mod aura { pub mod widget { pub use aura_widget::*; } pub use aura_widget::Renderer; pub use aura_widget::core::*; }
//! # pub type Element<'a, Message> = aura_widget::core::Element<'a, Message, aura_widget::Theme, aura_widget::Renderer>;
//! use aura::widget::{bottom_sheet, stack, text};
//!
//! #[derive(Clone)]
//! enum Message {
//!     SheetDismissed,
//!     SheetResized(usize),
//! }
//!
//! # struct State { is_open: bool, detent: usize }
//! fn view(state: &State) -> Element<'_, Message> {
//!     stack![
//!         text("the application"),
//!         bottom_sheet(text("sheet contents"), state.is_open)
//!             .detents([0.4, 0.9])
//!             .detent(state.detent)
//!             .on_detent_change(Message::SheetResized)
//!             .on_dismiss(Message::SheetDismissed),
//!     ]
//!     .into()
//! }
//! ```
//!
//! # Detents
//!
//! A sheet rests at one of several heights -- its *detents* -- given as
//! fractions of the space available to it. Dragging moves the sheet
//! freely; releasing snaps it to whichever detent the gesture was
//! heading for, or dismisses it if the gesture was heading past the
//! smallest one. This is what separates a sheet from a dialog that
//! happens to sit at the bottom.
//!
//! Like [`Drawer`](crate::drawer::Drawer), the sheet is controlled: the
//! application owns both `is_open` and the current detent, and the widget
//! reports intent. Only the animation between states lives in widget
//! state.

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

/// The fraction of the available height a sheet rests at when the caller
/// names no detents of its own.
const DEFAULT_DETENT: f32 = 0.5;

/// How far below its smallest detent a sheet must be dragged, as a
/// fraction of that detent's height, before releasing dismisses it.
const DISMISS_FRACTION: f32 = 0.5;

/// Downward speed, in logical pixels per second, past which a drag is a
/// fling and its direction alone decides the outcome.
const FLING_VELOCITY: f32 = 400.0;

/// The height of the drag handle's hit area. The visible grabber is
/// smaller; this is the region that actually accepts the gesture, and it
/// clears the 48px touch-target policy.
const HANDLE_HIT_HEIGHT: f32 = 48.0;

/// The visible grabber's dimensions.
const GRABBER_SIZE: Size = Size {
    width: 32.0,
    height: 4.0,
};

/// An in-progress vertical drag of the sheet.
#[derive(Debug, Clone, Copy)]
struct Drag {
    /// Where the pointer went down.
    origin: Point,
    /// The most recent pointer position.
    current: Point,
    /// The most recent position that produced a velocity sample.
    last: Point,
    /// When [`Drag::last`] was recorded.
    last_at: Instant,
    /// Downward pointer speed in pixels per second. Positive is toward
    /// the bottom of the screen, which is the dismissing direction.
    velocity: f32,
}

impl Drag {
    /// How far the sheet has been dragged down from its resting height.
    /// Negative values -- dragging up past the current detent -- are
    /// allowed so a sheet can be flicked to a taller detent.
    fn offset(&self) -> f32 {
        self.current.y - self.origin.y
    }
}

/// The widget state of a [`BottomSheet`].
#[derive(Debug)]
pub struct State {
    /// Drives the slide between dismissed and presented.
    animation: Animation<bool>,
    /// The in-progress drag, if any.
    drag: Option<Drag>,
    /// The most recent frame time.
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

/// A modal panel that slides up from the bottom of the screen.
#[allow(missing_debug_implementations)]
pub struct BottomSheet<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    content: Element<'a, Message, Theme, Renderer>,
    is_open: bool,
    detents: Vec<f32>,
    detent: usize,
    padding: Padding,
    show_handle: bool,
    on_dismiss: Option<Message>,
    on_detent_change: Option<Box<dyn Fn(usize) -> Message + 'a>>,
    dismiss_on_scrim: bool,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> BottomSheet<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// Creates a sheet around `content`, presented or dismissed as given.
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>, is_open: bool) -> Self {
        Self {
            content: content.into(),
            is_open,
            detents: vec![DEFAULT_DETENT],
            detent: 0,
            padding: Padding::new(16.0),
            show_handle: true,
            on_dismiss: None,
            on_detent_change: None,
            dismiss_on_scrim: true,
            class: Theme::default(),
        }
    }

    /// Sets the heights the sheet may rest at, as fractions of the space
    /// available to it.
    ///
    /// Values are clamped into `(0, 1]` and sorted, so a caller may pass
    /// them in any order. An empty iterator leaves the default single
    /// detent, because a sheet with nowhere to rest could not be shown.
    pub fn detents(mut self, detents: impl IntoIterator<Item = f32>) -> Self {
        let mut detents: Vec<f32> = detents
            .into_iter()
            .filter(|fraction| fraction.is_finite() && *fraction > 0.0)
            .map(|fraction| fraction.min(1.0))
            .collect();

        detents.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        detents.dedup();

        if !detents.is_empty() {
            self.detents = detents;
        }

        self
    }

    /// Sets which detent the sheet is currently resting at.
    ///
    /// Out-of-range indices clamp to the tallest detent rather than
    /// panicking: a sheet is presentation, and a stale index from
    /// application state should not take the window down.
    pub fn detent(mut self, detent: usize) -> Self {
        self.detent = detent;
        self
    }

    /// Sets the interior padding.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets whether the drag handle is drawn.
    ///
    /// The handle is the sheet's only *visible* affordance that it can be
    /// resized, so hiding it is appropriate only when the sheet has a
    /// single detent and cannot be dragged anywhere useful.
    pub fn show_handle(mut self, show_handle: bool) -> Self {
        self.show_handle = show_handle;
        self
    }

    /// Produces `on_dismiss` on Escape, a scrim tap, or a drag that
    /// closes the sheet.
    pub fn on_dismiss(mut self, on_dismiss: Message) -> Self {
        self.on_dismiss = Some(on_dismiss);
        self
    }

    /// Produces a message when a drag settles on a different detent.
    pub fn on_detent_change(mut self, on_detent_change: impl Fn(usize) -> Message + 'a) -> Self {
        self.on_detent_change = Some(Box::new(on_detent_change));
        self
    }

    /// Sets whether tapping the scrim dismisses the sheet.
    pub fn dismiss_on_scrim(mut self, dismiss: bool) -> Self {
        self.dismiss_on_scrim = dismiss;
        self
    }

    /// Sets the style of the sheet.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Whether the user can dismiss this sheet.
    pub const fn is_dismissible(&self) -> bool {
        self.on_dismiss.is_some()
    }

    /// The detent index actually in effect, with an out-of-range caller
    /// index clamped to the tallest.
    fn current_detent(&self) -> usize {
        self.detent.min(self.detents.len() - 1)
    }

    /// The sheet's resting height at its current detent.
    fn resting_height(&self, available: Size) -> f32 {
        self.detents[self.current_detent()] * available.height
    }

    /// Where the sheet sits, given the entry progress and any drag.
    fn surface_bounds(&self, available: Size, progress: f32, drag_offset: f32) -> Rectangle {
        let resting = self.resting_height(available);

        // Dragging up past the current detent grows the sheet rather than
        // lifting it off the bottom edge, which is what makes a sheet
        // feel anchored while a drawer feels carried.
        let height = (resting - drag_offset.min(0.0)).clamp(0.0, available.height);

        // Dragging down slides it off the bottom; the entry animation
        // does the same thing from `progress`.
        let hidden = drag_offset.max(0.0) + (1.0 - progress) * height;

        Rectangle {
            x: 0.0,
            y: available.height - height + hidden,
            width: available.width,
            height,
        }
    }

    /// The detent nearest `height`, which is where a slow drag settles.
    fn nearest_detent(&self, available: Size, height: f32) -> usize {
        let mut best = 0;
        let mut best_distance = f32::INFINITY;

        for (index, fraction) in self.detents.iter().enumerate() {
            let distance = (fraction * available.height - height).abs();

            if distance < best_distance {
                best_distance = distance;
                best = index;
            }
        }

        best
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for BottomSheet<'_, Message, Theme, Renderer>
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
        let available = limits.resolve(Length::Fill, Length::Fill, Size::ZERO);

        let state = tree.state.downcast_ref::<State>();
        let progress = state.animation.interpolate(0.0, 1.0, state.now);
        let drag_offset = state.drag.map_or(0.0, |drag| drag.offset());

        let surface = self.surface_bounds(available, progress, drag_offset);

        let handle_height = if self.show_handle {
            HANDLE_HIT_HEIGHT
        } else {
            0.0
        };

        let content_limits = layout::Limits::new(
            Size::ZERO,
            Size::new(surface.width, (surface.height - handle_height).max(0.0)),
        )
        .shrink(self.padding);

        let content = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, &content_limits)
            .move_to(Point::new(
                surface.x + self.padding.left,
                surface.y + handle_height + self.padding.top,
            ));

        // Three children: the sheet rectangle, the handle's hit area, and
        // the content. The first two carry no widget -- they exist so
        // `draw` and hit-testing can tell the sheet, its grabber, and the
        // scrim apart.
        layout::Node::with_children(
            available,
            vec![
                layout::Node::new(Size::new(surface.width, surface.height))
                    .move_to(Point::new(surface.x, surface.y)),
                layout::Node::new(Size::new(surface.width, handle_height))
                    .move_to(Point::new(surface.x, surface.y)),
                content,
            ],
        )
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Node, Role, State as SemanticState};

        let state = tree.state.downcast_ref::<State>();
        let progress = state.animation.interpolate(0.0, 1.0, state.now);

        // A dismissed sheet is absent, not merely off screen.
        if progress <= 0.0 && state.drag.is_none() {
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
        if let Some(content_layout) = layout.children().nth(2) {
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
            if let Some(content_layout) = layout.children().nth(2) {
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
        let available = bounds.size();

        let mut children = layout.children();
        let surface = children.next().map(|node| node.bounds());
        let handle = children.next().map(|node| node.bounds());
        let content_layout = children.next();

        {
            let state = tree.state.downcast_mut::<State>();

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

        let (progress, dragging) = {
            let state = tree.state.downcast_ref::<State>();

            (
                state.animation.interpolate(0.0, 1.0, state.now),
                state.drag.is_some(),
            )
        };

        let is_present = progress > 0.0 || dragging;

        if is_present {
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
                if is_present && *key == keyboard::Key::Named(keyboard::key::Named::Escape) =>
            {
                if let Some(on_dismiss) = self.on_dismiss.clone() {
                    shell.publish(on_dismiss);
                    shell.capture_event();
                }
            }

            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. })
                if is_present =>
            {
                let Some(position) = cursor.position_over(bounds) else {
                    return;
                };

                // Only the handle starts a drag. Letting the whole sheet
                // drag would fight any scrollable inside it for the same
                // vertical gesture, and the scrollable would lose.
                if handle.is_some_and(|handle| handle.contains(position)) {
                    let state = tree.state.downcast_mut::<State>();

                    state.drag = Some(Drag {
                        origin: position,
                        current: position,
                        last: position,
                        last_at: state.now,
                        velocity: 0.0,
                    });

                    shell.capture_event();
                } else if !surface.is_some_and(|surface| surface.contains(position)) {
                    if self.dismiss_on_scrim
                        && let Some(on_dismiss) = self.on_dismiss.clone()
                    {
                        shell.publish(on_dismiss);
                    }

                    shell.capture_event();
                }
            }

            Event::Mouse(mouse::Event::CursorMoved { position })
            | Event::Touch(touch::Event::FingerMoved { position, .. }) => {
                let state = tree.state.downcast_mut::<State>();

                if let Some(drag) = &mut state.drag {
                    drag.current = *position;

                    let elapsed = state
                        .now
                        .saturating_duration_since(drag.last_at)
                        .as_secs_f32();

                    if elapsed > 0.0 {
                        drag.velocity = (position.y - drag.last.y) / elapsed;
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
                self.end_drag(tree, available, shell);
            }

            _ => {
                if is_present
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

        if state.animation.interpolate(0.0, 1.0, state.now) <= 0.0 && state.drag.is_none() {
            return mouse::Interaction::None;
        }

        let mut children = layout.children();
        let _surface = children.next();
        let handle = children.next().map(|node| node.bounds());

        if self.show_handle && handle.is_some_and(|handle| cursor.is_over(handle)) {
            return mouse::Interaction::Grab;
        }

        children
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
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let progress = state.animation.interpolate(0.0, 1.0, state.now);

        if progress <= 0.0 && state.drag.is_none() {
            return;
        }

        let appearance = theme.style(&self.class);
        let mut children = layout.children();

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

        let surface = children.next();

        if let Some(surface) = surface {
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

        if let Some(handle) = children.next()
            && self.show_handle
        {
            let handle = handle.bounds();

            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: handle.x + (handle.width - GRABBER_SIZE.width) / 2.0,
                        y: handle.y + (handle.height - GRABBER_SIZE.height) / 2.0,
                        width: GRABBER_SIZE.width,
                        height: GRABBER_SIZE.height,
                    },
                    border: Border {
                        radius: (GRABBER_SIZE.height / 2.0).into(),
                        ..Border::default()
                    },
                    ..renderer::Quad::default()
                },
                appearance.handle,
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
        let content_layout = layout.children().nth(2)?;

        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            content_layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<Message, Theme, Renderer> BottomSheet<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// Ends a drag and decides where the sheet settles.
    ///
    /// A downward fling dismisses outright. Otherwise the sheet snaps to
    /// whichever detent it is nearest, or dismisses if it was dragged
    /// well below the smallest one.
    fn end_drag(&self, tree: &mut Tree, available: Size, shell: &mut Shell<'_, Message>) {
        let state = tree.state.downcast_mut::<State>();

        let Some(drag) = state.drag.take() else {
            return;
        };

        let resting = self.detents[self.current_detent()] * available.height;
        let visible = (resting - drag.offset()).max(0.0);
        let smallest = self.detents[0] * available.height;

        let dismissed = if drag.velocity >= FLING_VELOCITY {
            true
        } else if drag.velocity <= -FLING_VELOCITY {
            false
        } else {
            visible < smallest * DISMISS_FRACTION
        };

        if dismissed {
            if let Some(on_dismiss) = self.on_dismiss.clone() {
                shell.publish(on_dismiss);
            }

            state.animation.go_mut(false, state.now);
        } else {
            let settled = self.nearest_detent(available, visible);

            if settled != self.current_detent()
                && let Some(on_detent_change) = &self.on_detent_change
            {
                shell.publish(on_detent_change(settled));
            }
        }

        shell.capture_event();
        shell.request_redraw();
    }
}

impl<'a, Message, Theme, Renderer> From<BottomSheet<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a + Catalog,
    Renderer: 'a + crate::core::Renderer,
{
    fn from(sheet: BottomSheet<'a, Message, Theme, Renderer>) -> Self {
        Self::new(sheet)
    }
}

/// The appearance of a [`BottomSheet`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The scrim covering the page behind, at full opacity.
    pub scrim: Background,
    /// The sheet's fill.
    pub background: Background,
    /// The colour the sheet's content inherits.
    pub text_color: Color,
    /// The drag handle's fill.
    pub handle: Background,
    /// The border around the sheet.
    pub border: Border,
    /// The shadow the sheet casts over the page.
    pub shadow: Shadow,
}

/// The theme catalog of a [`BottomSheet`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for a [`BottomSheet`].
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

/// The default styling of a [`BottomSheet`] for aura's own theme.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        scrim: Background::Color(Color {
            a: 0.4,
            ..Color::BLACK
        }),
        background: Background::Color(palette.background.base.color),
        text_color: palette.background.base.text,
        handle: Background::Color(Color {
            a: 0.4,
            ..palette.background.base.text
        }),
        // Only the top corners round: the others meet the screen edge.
        border: Border {
            radius: crate::core::border::Radius::default()
                .top_left(20.0)
                .top_right(20.0),
            ..Border::default()
        },
        shadow: Shadow {
            color: Color {
                a: 0.25,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, -2.0),
            blur_radius: 16.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet<'a>(is_open: bool) -> BottomSheet<'a, ()> {
        BottomSheet::new(crate::text::Text::new("sheet"), is_open)
    }

    const AVAILABLE: Size = Size {
        width: 400.0,
        height: 800.0,
    };

    #[test]
    fn detents_are_sorted_clamped_and_deduplicated() {
        let sheet = sheet(true).detents([0.9, 0.25, 0.9, 3.0, -1.0, f32::NAN]);

        assert_eq!(sheet.detents, vec![0.25, 0.9, 1.0]);
    }

    #[test]
    fn an_empty_detent_list_keeps_a_usable_default() {
        let sheet = sheet(true).detents([]);

        assert_eq!(sheet.detents, vec![DEFAULT_DETENT]);
    }

    #[test]
    fn an_out_of_range_detent_clamps_instead_of_panicking() {
        let sheet = sheet(true).detents([0.3, 0.6]).detent(99);

        assert_eq!(sheet.current_detent(), 1);
        assert_eq!(sheet.resting_height(AVAILABLE), 0.6 * AVAILABLE.height);
    }

    #[test]
    fn a_dismissed_sheet_sits_entirely_below_the_screen() {
        let bounds = sheet(false).surface_bounds(AVAILABLE, 0.0, 0.0);

        assert!(
            bounds.y >= AVAILABLE.height,
            "a fully dismissed sheet must not be visible, got y={}",
            bounds.y
        );
    }

    #[test]
    fn a_presented_sheet_rests_at_its_detent_height() {
        let sheet = sheet(true).detents([0.5]);
        let bounds = sheet.surface_bounds(AVAILABLE, 1.0, 0.0);

        assert_eq!(bounds.height, 0.5 * AVAILABLE.height);
        assert_eq!(bounds.y, AVAILABLE.height - bounds.height);
    }

    #[test]
    fn dragging_down_slides_the_sheet_without_shrinking_it() {
        let sheet = sheet(true).detents([0.5]);
        let bounds = sheet.surface_bounds(AVAILABLE, 1.0, 100.0);

        assert_eq!(bounds.height, 0.5 * AVAILABLE.height);
        assert_eq!(bounds.y, AVAILABLE.height - bounds.height + 100.0);
    }

    #[test]
    fn dragging_up_grows_the_sheet_without_lifting_it() {
        let sheet = sheet(true).detents([0.5]);
        let bounds = sheet.surface_bounds(AVAILABLE, 1.0, -100.0);

        assert_eq!(bounds.height, 0.5 * AVAILABLE.height + 100.0);
        assert_eq!(
            bounds.y + bounds.height,
            AVAILABLE.height,
            "the sheet must stay anchored to the bottom edge"
        );
    }

    #[test]
    fn a_drag_settles_on_the_nearest_detent() {
        let sheet = sheet(true).detents([0.25, 0.5, 0.9]);

        assert_eq!(sheet.nearest_detent(AVAILABLE, 0.26 * AVAILABLE.height), 0);
        assert_eq!(sheet.nearest_detent(AVAILABLE, 0.45 * AVAILABLE.height), 1);
        assert_eq!(sheet.nearest_detent(AVAILABLE, 0.95 * AVAILABLE.height), 2);
    }
}
