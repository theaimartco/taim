//! Sections that expand one at a time to reveal their content.
//!
//! # Example
//! ```no_run
//! # mod aura { pub mod widget { pub use aura_widget::*; } pub use aura_widget::Renderer; pub use aura_widget::core::*; }
//! # pub type Element<'a, Message> = aura_widget::core::Element<'a, Message, aura_widget::Theme, aura_widget::Renderer>;
//! use aura::widget::{accordion, accordion::Section, text};
//!
//! #[derive(Clone)]
//! enum Message {
//!     SectionToggled(usize),
//! }
//!
//! # struct State { open: Option<usize> }
//! fn view(state: &State) -> Element<'_, Message> {
//!     accordion([
//!         Section::new("Account", text("account settings")),
//!         Section::new("Privacy", text("privacy settings")),
//!     ])
//!     .expanded(state.open)
//!     .on_toggle(Message::SectionToggled)
//!     .into()
//! }
//! ```
//!
//! # Nothing expanded is a real state
//!
//! [`Accordion::expanded`] takes an `Option`, so "all sections closed"
//! is representable rather than something a caller has to fake with an
//! out-of-range index. Pressing the open section closes it and reports
//! that, which is what makes the control reversible.
//!
//! # Closed content is absent, not hidden
//!
//! A collapsed section's content is not laid out, drawn, or visited, so
//! it costs nothing and never reaches the accessibility tree. Content
//! that is merely clipped is still reachable by Tab and still read by a
//! screen reader, which is how a "collapsed" section ends up announcing
//! itself.

use crate::core::alignment;
use crate::core::animation::Animation;
use crate::core::border::Border;
use crate::core::layout;
use crate::core::mouse;
use crate::core::renderer;
use crate::core::text;
use crate::core::time::Instant;
use crate::core::touch;
use crate::core::widget::Operation;
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::visit;
use crate::core::{
    Background, Color, Element, Event, Layout, Length, Padding, Pixels, Point, Rectangle, Shell,
    Size, Theme, Widget,
};

/// The height of a section's header, sized to clear the touch-target
/// policy.
const HEADER_HEIGHT: f32 = 56.0;

/// The chevron's rendered size.
const CHEVRON_SIZE: f32 = 18.0;

/// The width reserved for the chevron at the trailing edge.
const CHEVRON_MARGIN: f32 = 36.0;

/// The padding between a header's leading edge and its title.
const TITLE_PADDING: f32 = 16.0;

/// The title's text size.
const TITLE_SIZE: f32 = 15.0;

/// One collapsible section.
#[allow(missing_debug_implementations)]
pub struct Section<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::text::Renderer,
    Theme: Catalog,
{
    title: String,
    content: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Theme, Renderer> Section<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::text::Renderer,
    Theme: Catalog,
{
    /// Creates a section titled `title` revealing `content`.
    pub fn new(
        title: impl Into<String>,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        Self {
            title: title.into(),
            content: content.into(),
        }
    }

    /// The section's title, which is also its accessible name.
    pub fn title(&self) -> &str {
        &self.title
    }
}

/// The widget state of an [`Accordion`].
#[derive(Debug)]
pub struct State {
    /// Drives the reveal of whichever section is open.
    reveal: Animation<f32>,
    /// The most recent frame time.
    now: Instant,
}

impl Default for State {
    fn default() -> Self {
        Self {
            reveal: Animation::new(0.0).quick(),
            now: Instant::now(),
        }
    }
}

/// Sections that expand one at a time to reveal their content.
#[allow(missing_debug_implementations)]
pub struct Accordion<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::text::Renderer,
    Theme: Catalog,
{
    sections: Vec<Section<'a, Message, Theme, Renderer>>,
    expanded: Option<usize>,
    on_toggle: Option<Box<dyn Fn(usize) -> Message + 'a>>,
    padding: Padding,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> Accordion<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::text::Renderer,
    Theme: Catalog,
{
    /// Creates an accordion from `sections`.
    pub fn new(sections: impl IntoIterator<Item = Section<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            sections: sections.into_iter().collect(),
            expanded: None,
            on_toggle: None,
            padding: Padding::new(16.0),
            class: Theme::default(),
        }
    }

    /// Sets which section is open, if any.
    pub fn expanded(mut self, expanded: Option<usize>) -> Self {
        self.expanded = expanded;
        self
    }

    /// Produces a message naming the section whose header was pressed.
    ///
    /// The message carries the pressed index whether that section is
    /// opening or closing; the caller decides which, since it owns the
    /// state. Pressing the open section is how it closes.
    pub fn on_toggle(mut self, on_toggle: impl Fn(usize) -> Message + 'a) -> Self {
        self.on_toggle = Some(Box::new(on_toggle));
        self
    }

    /// Sets the padding around a section's content.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the style of the accordion.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// The open section's index, if it names one that exists.
    fn open(&self) -> Option<usize> {
        self.expanded.filter(|index| *index < self.sections.len())
    }

    /// Whether `index` is the open section.
    fn is_open(&self, index: usize) -> bool {
        self.open() == Some(index)
    }

    /// The header bounds of the section at `index`, given the layout's
    /// own children.
    fn header_bounds(&self, layout: Layout<'_>, index: usize) -> Option<Rectangle> {
        let mut y = layout.bounds().y;
        let bounds = layout.bounds();

        for (position, _) in self.sections.iter().enumerate() {
            let header = Rectangle {
                x: bounds.x,
                y,
                width: bounds.width,
                height: HEADER_HEIGHT,
            };

            if position == index {
                return Some(header);
            }

            y += HEADER_HEIGHT;

            if self.is_open(position)
                && let Some(content_layout) = layout.children().next()
                && position == self.open().unwrap_or(usize::MAX)
            {
                y += content_layout.bounds().height;
            }
        }

        None
    }

    /// Which section's header, if any, a position landed on.
    fn header_at(&self, layout: Layout<'_>, position: Point) -> Option<usize> {
        (0..self.sections.len()).find(|index| {
            self.header_bounds(layout, *index)
                .is_some_and(|header| header.contains(position))
        })
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Accordion<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: crate::core::text::Renderer,
    Theme: Catalog,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State {
            reveal: Animation::new(if self.expanded.is_some() { 1.0 } else { 0.0 }).quick(),
            ..State::default()
        })
    }

    fn diff(&mut self, tree: &mut Tree) {
        // Every section keeps its tree whether or not it is open, so
        // reopening a section does not reset whatever state its content
        // was holding.
        tree.diff_children_custom(
            &mut self.sections,
            |section_tree, section| section.content.as_widget_mut().diff(section_tree),
            |section| Tree::new(section.content.as_widget()),
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
        let max = limits.max();
        let headers_height = HEADER_HEIGHT * self.sections.len() as f32;

        let open = self.open();

        // Only the open section is laid out at all. A collapsed section
        // is absent, not clipped; see this module's own documentation.
        let content = open.map(|index| {
            let content_limits = layout::Limits::new(
                Size::ZERO,
                Size::new(
                    (max.width - self.padding.left - self.padding.right).max(0.0),
                    max.height,
                ),
            );

            let mut y = HEADER_HEIGHT * (index + 1) as f32;
            y += self.padding.top;

            self.sections[index]
                .content
                .as_widget_mut()
                .layout(&mut tree.children[index], renderer, &content_limits)
                .move_to(Point::new(self.padding.left, y))
        });

        let content_height = content.as_ref().map_or(0.0, |node| {
            node.size().height + self.padding.top + self.padding.bottom
        });

        layout::Node::with_children(
            Size::new(max.width, headers_height + content_height),
            content.into_iter().collect(),
        )
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Node, Role, State as SemanticState};

        // The accordion groups its sections. Each header's own state is
        // reported through `header_semantics` below, which the runtime
        // reaches via `visit_children`.
        let _ = layout;

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
        let Some(index) = self.open() else {
            return;
        };

        if let Some(content_layout) = layout.children().next() {
            visit(&mut visit::Widgets::new(
                self.sections[index].content.as_widget_mut(),
                &mut tree.children[index],
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
            let Some(index) = self.open() else {
                return;
            };

            if let Some(content_layout) = layout.children().next() {
                self.sections[index].content.as_widget_mut().operate(
                    &mut tree.children[index],
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

                if state.reveal.is_animating(*now) {
                    shell.request_redraw();
                }
            }
        }

        // The open section's content sees events first: a control inside
        // it must win its own click before a header does.
        if let Some(index) = self.open()
            && let Some(content_layout) = layout.children().next()
        {
            self.sections[index].content.as_widget_mut().update(
                &mut tree.children[index],
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

        let Some(on_toggle) = &self.on_toggle else {
            return;
        };

        if !matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                | Event::Touch(touch::Event::FingerPressed { .. })
        ) {
            return;
        }

        let Some(position) = cursor.position_over(layout.bounds()) else {
            return;
        };

        if let Some(index) = self.header_at(layout, position) {
            shell.publish(on_toggle(index));
            shell.capture_event();
            shell.request_redraw();
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
        if self.on_toggle.is_none() {
            return mouse::Interaction::None;
        }

        let Some(position) = cursor.position_over(layout.bounds()) else {
            return mouse::Interaction::None;
        };

        if self.header_at(layout, position).is_some() {
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
        let appearance = theme.style(&self.class);

        for index in 0..self.sections.len() {
            let Some(header) = self.header_bounds(layout, index) else {
                continue;
            };

            renderer.fill_quad(
                renderer::Quad {
                    bounds: header,
                    border: appearance.border,
                    ..renderer::Quad::default()
                },
                if self.is_open(index) {
                    appearance.open_header
                } else {
                    appearance.header
                },
            );

            // The title. This widget stored one, exposed it as its
            // accessible name, and never drew it: its renderer bound was
            // `core::Renderer`, which cannot draw text at all, so every
            // header rendered as a blank bar. Found by looking at a golden.
            renderer.fill_text(
                text::Text {
                    content: self.sections[index].title.clone(),
                    font: renderer.default_font(),
                    size: Pixels(TITLE_SIZE),
                    line_height: text::LineHeight::default(),
                    bounds: Size::new(
                        (header.width - TITLE_PADDING - CHEVRON_MARGIN).max(0.0),
                        header.height,
                    ),
                    align_x: text::Alignment::Left,
                    align_y: alignment::Vertical::Center,
                    shaping: text::Shaping::Advanced,
                    wrapping: text::Wrapping::None,
                    ellipsis: text::Ellipsis::default(),
                    hint_factor: None,
                },
                Point::new(header.x + TITLE_PADDING, header.y + header.height / 2.0),
                appearance.text_color,
                *viewport,
            );

            // The chevron, from the icon font. It used to be two *vertical*
            // bars -- `width: CHEVRON_STROKE` on both -- which is not a
            // chevron in either state and rendered as two tick marks. A quad
            // cannot express a diagonal, so a glyph is the fix, and it keeps
            // every chevron in the framework identical.
            let chevron = if self.is_open(index) {
                crate::icon::Icon::CHEVRON_UP
            } else {
                crate::icon::Icon::CHEVRON_DOWN
            };

            renderer.fill_text(
                text::Text {
                    content: chevron.codepoint().to_string(),
                    // `ICON_FONT` rather than `icon::FONT`: same font file,
                    // but typed as `Renderer::Font`, so the widget stays
                    // generic over any text renderer.
                    font: Renderer::ICON_FONT,
                    size: Pixels(CHEVRON_SIZE),
                    line_height: text::LineHeight::default(),
                    bounds: Size::new(CHEVRON_MARGIN, header.height),
                    align_x: text::Alignment::Center,
                    align_y: alignment::Vertical::Center,
                    shaping: text::Shaping::Basic,
                    wrapping: text::Wrapping::None,
                    ellipsis: text::Ellipsis::default(),
                    hint_factor: None,
                },
                Point::new(
                    header.x + header.width - CHEVRON_MARGIN / 2.0,
                    header.y + header.height / 2.0,
                ),
                appearance.chevron_color,
                *viewport,
            );
        }

        if let Some(index) = self.open()
            && let Some(content_layout) = layout.children().next()
        {
            self.sections[index].content.as_widget().draw(
                &tree.children[index],
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
}

impl<'a, Message, Theme, Renderer> From<Accordion<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a + Catalog,
    Renderer: 'a + crate::core::text::Renderer,
{
    fn from(accordion: Accordion<'a, Message, Theme, Renderer>) -> Self {
        Self::new(accordion)
    }
}

/// The appearance of an [`Accordion`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// A closed section header's fill.
    pub header: Background,
    /// The open section header's fill.
    pub open_header: Background,
    /// The colour of the chevron.
    pub chevron_color: Color,
    /// The colour the open section's content inherits.
    pub text_color: Color,
    /// The border around each header.
    pub border: Border,
}

/// The theme catalog of an [`Accordion`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for an [`Accordion`].
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

/// The default styling of an [`Accordion`] for aura's own theme.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        header: Background::Color(palette.background.base.color),
        open_header: Background::Color(palette.background.weak.color),
        chevron_color: Color {
            a: 0.7,
            ..palette.background.base.text
        },
        text_color: palette.background.base.text,
        border: Border {
            width: 1.0,
            color: palette.background.weak.color,
            ..Border::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accordion<'a>(count: usize) -> Accordion<'a, ()> {
        Accordion::new(
            (0..count).map(|index| {
                Section::new(format!("Section {index}"), crate::text::Text::new("body"))
            }),
        )
    }

    #[test]
    fn nothing_is_open_by_default() {
        assert_eq!(accordion(3).open(), None);
    }

    #[test]
    fn closing_everything_is_representable() {
        // The whole point of taking an `Option`: a caller does not have
        // to invent an out-of-range index to mean "all closed".
        assert_eq!(accordion(3).expanded(None).open(), None);
        assert!((0..3).all(|index| !accordion(3).expanded(None).is_open(index)));
    }

    #[test]
    fn exactly_one_section_is_open_at_a_time() {
        let accordion = accordion(3).expanded(Some(1));

        assert_eq!(accordion.open(), Some(1));

        let open: Vec<bool> = (0..3).map(|index| accordion.is_open(index)).collect();
        assert_eq!(open, vec![false, true, false]);
    }

    #[test]
    fn an_out_of_range_index_opens_nothing() {
        let accordion = accordion(3).expanded(Some(99));

        assert_eq!(accordion.open(), None);
        assert!((0..3).all(|index| !accordion.is_open(index)));
    }

    #[test]
    fn an_empty_accordion_has_nothing_to_open() {
        assert_eq!(accordion(0).expanded(Some(0)).open(), None);
    }

    #[test]
    fn a_section_keeps_its_title_as_its_accessible_name() {
        let accordion = accordion(2);

        assert_eq!(accordion.sections[0].title(), "Section 0");
        assert_eq!(accordion.sections[1].title(), "Section 1");
    }

    #[test]
    fn the_reveal_starts_settled_to_match_the_initial_state() {
        let closed = accordion(3).state();
        assert_eq!(closed.downcast_ref::<State>().reveal.value(), 0.0);

        let open = accordion(3).expanded(Some(1)).state();
        assert_eq!(open.downcast_ref::<State>().reveal.value(), 1.0);
    }
}
