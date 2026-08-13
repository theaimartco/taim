//! A row of mutually exclusive options sharing one track.
//!
//! # Example
//! ```no_run
//! # mod aura { pub mod widget { pub use aura_widget::*; } pub use aura_widget::Renderer; pub use aura_widget::core::*; }
//! # pub type Element<'a, Message> = aura_widget::core::Element<'a, Message, aura_widget::Theme, aura_widget::Renderer>;
//! use aura::widget::{segmented_control, segmented_control::Segment, text};
//!
//! #[derive(Clone)]
//! enum Message {
//!     ViewChanged(usize),
//! }
//!
//! # struct State { view: usize }
//! fn view(state: &State) -> Element<'_, Message> {
//!     segmented_control([
//!         Segment::new(text("Day"), "Day").on_press(Message::ViewChanged(0)),
//!         Segment::new(text("Week"), "Week").on_press(Message::ViewChanged(1)),
//!     ])
//!     .selected(state.view)
//!     .into()
//! }
//! ```
//!
//! # Why these are radio buttons, not tabs
//!
//! A segmented control and a [`tabs`](crate::tabs) bar look similar and
//! mean different things. Tabs switch between *views of a screen*;
//! segments choose *one value from a set*, which is a radio group.
//! Reporting [`Role::RadioButton`](aura_core::semantics::Role::RadioButton)
//! is what tells a screen-reader user "pick one of these three" rather
//! than "navigate to one of these three".
//!
//! # The selected segment slides
//!
//! One thumb moves between segments rather than each segment lighting up
//! in place, for the same reason the [`tabs`](crate::tabs) indicator
//! animates: the movement is what shows where the selection came from.

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
    Background, Color, Element, Event, Layout, Length, Padding, Point, Rectangle, Shell, Size,
    Theme, Vector, Widget,
};

/// The track's height, sized so each segment clears the touch-target
/// policy.
const DEFAULT_HEIGHT: f32 = 48.0;

/// The gap between the thumb and the track it slides inside.
const THUMB_INSET: f32 = 3.0;

/// One option within a [`SegmentedControl`].
#[allow(missing_debug_implementations)]
pub struct Segment<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    content: Element<'a, Message, Theme, Renderer>,
    label: String,
    on_press: Option<Message>,
    /// Pushed down by the control before layout and draw.
    selected: bool,
}

impl<'a, Message, Theme, Renderer> Segment<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// Creates a segment displaying `content`, announced as `label`.
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        label: impl Into<String>,
    ) -> Self {
        Self {
            content: content.into(),
            label: label.into(),
            on_press: None,
            selected: false,
        }
    }

    /// Sets the message produced when this segment is chosen.
    pub fn on_press(mut self, on_press: Message) -> Self {
        self.on_press = Some(on_press);
        self
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Segment<'_, Message, Theme, Renderer>
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
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let cell = limits.resolve(Length::Fill, Length::Fill, Size::ZERO);

        let content = self.content.as_widget_mut().layout(
            &mut tree.children[0],
            renderer,
            &layout::Limits::new(Size::ZERO, cell),
        );

        let size = content.size();

        let content = content.move_to(Point::new(
            (cell.width - size.width) / 2.0,
            (cell.height - size.height) / 2.0,
        ));

        layout::Node::with_children(cell, vec![content])
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Node, Role, State as SemanticState};

        let enabled = self.on_press.is_some();

        let mut flags = if enabled {
            SemanticState::FOCUSABLE
        } else {
            SemanticState::DISABLED
        };

        if self.selected {
            // Both: `CHECKED` is what a radio group is read by, and
            // `SELECTED` is what a visual highlight is read by. A segment
            // is both at once.
            flags |= SemanticState::CHECKED | SemanticState::SELECTED;
        }

        Some(
            Node::new(tree.id().clone(), layout.bounds(), Role::RadioButton)
                .with_name(self.label.as_str())
                .with_state(flags)
                .with_actions(if enabled {
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

        let Some(on_press) = self.on_press.clone() else {
            return;
        };

        // A segment that is already chosen does not re-publish. Choosing
        // the current value is a no-op, and republishing would make an
        // application that reloads on change reload for nothing.
        if self.selected {
            return;
        }

        if matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                | Event::Touch(touch::Event::FingerPressed { .. })
        ) && cursor.is_over(layout.bounds())
        {
            shell.publish(on_press);
            shell.capture_event();
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.on_press.is_some() && !self.selected && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
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
        let class = <Theme as Catalog>::default();
        let appearance = theme.style(&class);

        if let Some(content_layout) = layout.children().next() {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                &renderer::Style {
                    text_color: if self.selected {
                        appearance.selected_text_color
                    } else {
                        appearance.text_color
                    },
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

/// The widget state of a [`SegmentedControl`].
#[derive(Debug)]
pub struct State {
    /// The thumb's position, as a (fractional) segment index.
    thumb: Animation<f32>,
    /// The most recent frame time.
    now: Instant,
}

impl Default for State {
    fn default() -> Self {
        Self {
            thumb: Animation::new(0.0).quick(),
            now: Instant::now(),
        }
    }
}

/// A row of mutually exclusive options sharing one track.
#[allow(missing_debug_implementations)]
pub struct SegmentedControl<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    segments: Vec<Segment<'a, Message, Theme, Renderer>>,
    selected: usize,
    height: f32,
    padding: Padding,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> SegmentedControl<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// Creates a control from `segments`.
    pub fn new(segments: impl IntoIterator<Item = Segment<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            segments: segments.into_iter().collect(),
            selected: 0,
            height: DEFAULT_HEIGHT,
            padding: Padding::ZERO,
            class: Theme::default(),
        }
    }

    /// Sets which segment is chosen.
    ///
    /// An out-of-range index chooses nothing rather than clamping:
    /// highlighting the wrong value in an exclusive control is worse
    /// than highlighting none, because it is not visibly wrong.
    pub fn selected(mut self, selected: usize) -> Self {
        self.selected = selected;
        self
    }

    /// Sets the track's height.
    pub fn height(mut self, height: impl Into<crate::core::Pixels>) -> Self {
        self.height = height.into().0.max(0.0);
        self
    }

    /// Sets the padding around the segments.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the style of the control.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Whether the current selection names a segment that exists.
    fn has_selection(&self) -> bool {
        self.selected < self.segments.len()
    }

    /// Pushes the current selection down into the segments.
    fn sync_selection(&mut self) {
        for (index, segment) in self.segments.iter_mut().enumerate() {
            segment.selected = index == self.selected;
        }
    }

    /// The width of one segment, given the track's own width.
    fn segment_width(&self, track: Size) -> f32 {
        if self.segments.is_empty() {
            return 0.0;
        }

        let inner = (track.width - self.padding.left - self.padding.right).max(0.0);

        inner / self.segments.len() as f32
    }

    /// The thumb's bounds for a (possibly fractional) position.
    fn thumb_bounds(&self, track: Rectangle, position: f32) -> Rectangle {
        let width = self.segment_width(track.size());

        Rectangle {
            x: track.x + self.padding.left + position * width + THUMB_INSET,
            y: track.y + self.padding.top + THUMB_INSET,
            width: (width - 2.0 * THUMB_INSET).max(0.0),
            height: (track.height - self.padding.top - self.padding.bottom - 2.0 * THUMB_INSET)
                .max(0.0),
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for SegmentedControl<'_, Message, Theme, Renderer>
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
            thumb: Animation::new(self.selected as f32).quick(),
            ..State::default()
        })
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children_custom(
            &mut self.segments,
            |segment_tree, segment| segment.diff(segment_tree),
            |segment| {
                let mut segment_tree = Tree::empty();
                segment_tree.children = vec![Tree::new(segment.content.as_widget())];
                segment_tree
            },
        );
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.sync_selection();

        let track = limits.resolve(
            Length::Fill,
            Length::Fixed(self.height),
            Size::new(0.0, self.height),
        );

        let width = self.segment_width(track);
        let height = (track.height - self.padding.top - self.padding.bottom).max(0.0);

        let cell_limits = layout::Limits::new(Size::ZERO, Size::new(width, height));

        let children = self
            .segments
            .iter_mut()
            .zip(tree.children.iter_mut())
            .enumerate()
            .map(|(index, (segment, segment_tree))| {
                segment
                    .layout(segment_tree, renderer, &cell_limits)
                    .move_to(Point::new(
                        self.padding.left + index as f32 * width,
                        self.padding.top,
                    ))
            })
            .collect();

        layout::Node::with_children(track, children)
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Node, Role, State as SemanticState};

        // `NamedGroup` rather than `Group`: a radio group is a named set
        // of related controls, and that is exactly what platform APIs
        // expect to find wrapping radio buttons.
        Some(
            Node::new(tree.id().clone(), layout.bounds(), Role::NamedGroup)
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
        self.sync_selection();

        for ((segment, segment_tree), segment_layout) in self
            .segments
            .iter_mut()
            .zip(tree.children.iter_mut())
            .zip(layout.children())
        {
            visit(&mut visit::Widgets::new(
                segment,
                segment_tree,
                segment_layout,
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
            for ((segment, segment_tree), segment_layout) in self
                .segments
                .iter_mut()
                .zip(tree.children.iter_mut())
                .zip(layout.children())
            {
                segment.operate(segment_tree, segment_layout, renderer, operation);
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
        self.sync_selection();

        {
            let state = tree.state.downcast_mut::<State>();

            if self.has_selection() {
                let target = self.selected as f32;

                if (state.thumb.value() - target).abs() > f32::EPSILON {
                    state.thumb.go_mut(target, state.now);
                    shell.request_redraw();
                }
            }

            if let Event::Window(crate::core::window::Event::RedrawRequested(now)) = event {
                state.now = *now;

                if state.thumb.is_animating(*now) {
                    shell.request_redraw();
                }
            }
        }

        for ((segment, segment_tree), segment_layout) in self
            .segments
            .iter_mut()
            .zip(tree.children.iter_mut())
            .zip(layout.children())
        {
            segment.update(
                segment_tree,
                event,
                segment_layout,
                cursor,
                renderer,
                shell,
                viewport,
            );

            if shell.is_event_captured() {
                return;
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
        self.segments
            .iter()
            .zip(tree.children.iter())
            .zip(layout.children())
            .map(|((segment, segment_tree), segment_layout)| {
                segment.mouse_interaction(
                    segment_tree,
                    segment_layout,
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
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let appearance = theme.style(&self.class);
        let bounds = layout.bounds();

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border {
                    radius: (bounds.height / 2.0).into(),
                    ..appearance.border
                },
                ..renderer::Quad::default()
            },
            appearance.track,
        );

        // The thumb is drawn under the segments so their content sits on
        // top of it, and positioned from the animation so it lands
        // between two segments mid-slide.
        if self.has_selection() {
            let state = tree.state.downcast_ref::<State>();
            let thumb = self.thumb_bounds(bounds, state.thumb.interpolate(state.now));

            renderer.fill_quad(
                renderer::Quad {
                    bounds: thumb,
                    border: Border {
                        radius: (thumb.height / 2.0).into(),
                        ..Border::default()
                    },
                    ..renderer::Quad::default()
                },
                appearance.thumb,
            );
        }

        for ((segment, segment_tree), segment_layout) in self
            .segments
            .iter()
            .zip(tree.children.iter())
            .zip(layout.children())
        {
            segment.draw(
                segment_tree,
                renderer,
                theme,
                style,
                segment_layout,
                cursor,
                viewport,
            );
        }
    }
}

impl<'a, Message, Theme, Renderer> From<SegmentedControl<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a + Catalog,
    Renderer: 'a + crate::core::Renderer,
{
    fn from(control: SegmentedControl<'a, Message, Theme, Renderer>) -> Self {
        Self::new(control)
    }
}

/// The appearance of a [`SegmentedControl`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The track the thumb slides inside.
    pub track: Background,
    /// The thumb marking the chosen segment.
    pub thumb: Background,
    /// The colour of an unchosen segment's content.
    pub text_color: Color,
    /// The colour of the chosen segment's content.
    pub selected_text_color: Color,
    /// The border around the track.
    pub border: Border,
}

/// The theme catalog of a [`SegmentedControl`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for a [`SegmentedControl`].
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

/// The default styling of a [`SegmentedControl`] for aura's own theme.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        track: Background::Color(palette.background.weak.color),
        thumb: Background::Color(palette.background.base.color),
        text_color: Color {
            a: 0.7,
            ..palette.background.base.text
        },
        selected_text_color: palette.background.base.text,
        border: Border::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control<'a>(count: usize) -> SegmentedControl<'a, ()> {
        SegmentedControl::new((0..count).map(|index| {
            Segment::new(crate::text::Text::new("s"), format!("Segment {index}"))
        }))
    }

    const TRACK: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 300.0,
        height: DEFAULT_HEIGHT,
    };

    #[test]
    fn exactly_one_segment_is_chosen() {
        let mut control = control(3).selected(1);
        control.sync_selection();

        let chosen: Vec<bool> = control.segments.iter().map(|s| s.selected).collect();

        assert_eq!(chosen, vec![false, true, false]);
    }

    #[test]
    fn an_out_of_range_selection_chooses_nothing() {
        let mut control = control(3).selected(99);
        control.sync_selection();

        assert!(!control.has_selection());
        assert!(control.segments.iter().all(|s| !s.selected));
    }

    #[test]
    fn segments_divide_the_track_evenly() {
        assert_eq!(control(3).segment_width(TRACK.size()), 100.0);
        assert_eq!(control(2).segment_width(TRACK.size()), 150.0);
    }

    #[test]
    fn an_empty_control_does_not_divide_by_zero() {
        assert_eq!(control(0).segment_width(TRACK.size()), 0.0);
    }

    #[test]
    fn the_thumb_is_inset_inside_its_segment() {
        let control = control(3);
        let thumb = control.thumb_bounds(TRACK, 0.0);

        assert_eq!(thumb.x, TRACK.x + THUMB_INSET);
        assert_eq!(thumb.width, 100.0 - 2.0 * THUMB_INSET);
        assert!(thumb.height < TRACK.height, "the thumb sits inside the track");
    }

    #[test]
    fn the_thumb_moves_one_segment_width_per_step() {
        let control = control(3);

        let first = control.thumb_bounds(TRACK, 0.0);
        let second = control.thumb_bounds(TRACK, 1.0);

        assert_eq!(second.x - first.x, 100.0);
    }

    #[test]
    fn the_thumb_can_sit_between_two_segments_mid_slide() {
        let control = control(3);
        let halfway = control.thumb_bounds(TRACK, 0.5);

        assert_eq!(halfway.x, TRACK.x + 50.0 + THUMB_INSET);
    }

    #[test]
    fn the_thumb_starts_settled_on_the_initial_selection() {
        let control = control(3).selected(2);
        let state = control.state();
        let state = state.downcast_ref::<State>();

        assert_eq!(state.thumb.value(), 2.0);
        assert!(!state.thumb.is_animating(Instant::now()));
    }
}
