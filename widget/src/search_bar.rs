//! A search field with a leading glyph and a clear affordance.
//!
//! # Example
//! ```no_run
//! # mod taim { pub mod widget { pub use taim_widget::*; } pub use taim_widget::Renderer; pub use taim_widget::core::*; }
//! # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
//! use taim::widget::search_bar;
//!
//! #[derive(Clone)]
//! enum Message {
//!     QueryChanged(String),
//!     Submitted,
//!     Cleared,
//! }
//!
//! # struct State { query: String }
//! fn view(state: &State) -> Element<'_, Message> {
//!     search_bar("Search", &state.query)
//!         .on_input(Message::QueryChanged)
//!         .on_submit(Message::Submitted)
//!         .on_clear(Message::Cleared)
//!         .into()
//! }
//! ```
//!
//! # It wraps a `text_input` rather than reimplementing one
//!
//! Text editing is selection, IME composition, cursor movement, undo,
//! and platform key bindings -- some seventeen hundred lines of it in
//! [`text_input`](crate::text_input). A search field is that widget with
//! a glyph on one side and a clear button on the other, so this widget
//! owns the chrome and delegates every keystroke.
//!
//! The consequence for accessibility is the good one: the inner field
//! reports [`Role::TextField`](taim_core::semantics::Role::TextField)
//! with its own value and actions, and this widget reports nothing of
//! its own rather than wrapping it in a second, competing node.
//!
//! # The clear affordance appears only when there is something to clear
//!
//! An always-present clear button on an empty field is a control that
//! does nothing, occupying a touch target next to one that does.

use crate::core::border::Border;
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::touch;
use crate::core::widget::Operation;
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::visit;
use crate::core::{
    Background, Color, Element, Event, Layout, Length, Padding, Point, Rectangle, Shell, Size,
    Theme, Vector, Widget,
};

/// The bar's height, chosen so both affordances clear the touch-target
/// policy.
const DEFAULT_HEIGHT: f32 = 48.0;

/// The width reserved for the leading glyph and the trailing clear
/// region. Equal so the field itself stays optically centred.
const AFFORDANCE_WIDTH: f32 = 44.0;

/// The rendered size of the leading and trailing glyphs.
const GLYPH_SIZE: f32 = 18.0;

/// A search field with a leading glyph and a clear affordance.
#[allow(missing_debug_implementations)]
pub struct SearchBar<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::text::Renderer,
    Theme: Catalog + crate::text_input::Catalog,
{
    input: crate::text_input::TextInput<'a, Message, Theme, Renderer>,
    is_empty: bool,
    height: f32,
    padding: Padding,
    on_clear: Option<Message>,
    class: <Theme as Catalog>::Class<'a>,
}

impl<'a, Message, Theme, Renderer> SearchBar<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Renderer: crate::core::text::Renderer + 'a,
    Theme: Catalog + crate::text_input::Catalog + 'a,
{
    /// Creates a search bar showing `value`, prompting with `placeholder`.
    pub fn new(placeholder: &str, value: &str) -> Self
    where
        <Theme as crate::text_input::Catalog>::Class<'a>:
            From<crate::text_input::StyleFn<'a, Theme>>,
    {
        Self {
            is_empty: value.is_empty(),
            input: crate::text_input::TextInput::new(placeholder, value)
                .width(Length::Fill)
                // The chrome supplies the outline and the fill; a second
                // border drawn by the field inside it would read as a
                // box within a box. The theme's own field style is
                // resolved first and then flattened, rather than
                // replaced, so state colours -- focus, disabled, the
                // selection fill -- still come from the theme.
                .style(|theme: &Theme, status| {
                    let class = <Theme as crate::text_input::Catalog>::default();

                    let mut style =
                        <Theme as crate::text_input::Catalog>::style(theme, &class, status);

                    style.background = Background::Color(Color::TRANSPARENT);
                    style.border = Border::default();
                    style
                }),
            height: DEFAULT_HEIGHT,
            padding: Padding::ZERO,
            on_clear: None,
            class: <Theme as Catalog>::default(),
        }
    }

    /// Produces a message for each edit of the query.
    ///
    /// Without it the field is read-only, which is a legitimate state
    /// for a search bar showing a query someone else set.
    pub fn on_input(self, on_input: impl Fn(String) -> Message + 'a) -> Self {
        Self {
            input: self.input.on_input(on_input),
            ..self
        }
    }

    /// Produces `on_submit` when the query is committed.
    pub fn on_submit(self, on_submit: Message) -> Self {
        Self {
            input: self.input.on_submit(on_submit),
            ..self
        }
    }

    /// Produces `on_clear` when the trailing affordance is activated.
    ///
    /// The affordance is only drawn when there is a query to clear and
    /// a message to send.
    pub fn on_clear(mut self, on_clear: Message) -> Self {
        self.on_clear = Some(on_clear);
        self
    }

    /// Sets the bar's height.
    pub fn height(mut self, height: impl Into<crate::core::Pixels>) -> Self {
        self.height = height.into().0.max(0.0);
        self
    }

    /// Sets the padding around the bar's contents.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the style of the bar's chrome.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        <Theme as Catalog>::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }
}

impl<Message, Theme, Renderer> SearchBar<'_, Message, Theme, Renderer>
where
    Renderer: crate::core::text::Renderer,
    Theme: Catalog + crate::text_input::Catalog,
{
    /// Whether the clear affordance is currently offered.
    fn has_clear(&self) -> bool {
        self.on_clear.is_some() && !self.is_empty
    }

    /// The leading glyph's region.
    fn glyph_bounds(&self, bounds: Rectangle) -> Rectangle {
        Rectangle {
            x: bounds.x + self.padding.left,
            y: bounds.y,
            width: AFFORDANCE_WIDTH,
            height: bounds.height,
        }
    }

    /// The trailing clear region, when there is one.
    fn clear_bounds(&self, bounds: Rectangle) -> Option<Rectangle> {
        if !self.has_clear() {
            return None;
        }

        Some(Rectangle {
            x: bounds.x + bounds.width - self.padding.right - AFFORDANCE_WIDTH,
            y: bounds.y,
            width: AFFORDANCE_WIDTH,
            height: bounds.height,
        })
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for SearchBar<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: crate::core::text::Renderer,
    Theme: Catalog + crate::text_input::Catalog,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    fn state(&self) -> tree::State {
        tree::State::None
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children_custom(
            std::slice::from_mut(&mut self.input),
            |input_tree, input| Widget::diff(input, input_tree),
            |input| Tree::new(input as &dyn Widget<Message, Theme, Renderer>),
        );
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(self.height))
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let bar = limits.resolve(
            Length::Fill,
            Length::Fixed(self.height),
            Size::new(0.0, self.height),
        );

        let clear_width = if self.has_clear() {
            AFFORDANCE_WIDTH
        } else {
            0.0
        };

        let input_width =
            (bar.width - self.padding.left - self.padding.right - AFFORDANCE_WIDTH - clear_width)
                .max(0.0);

        let input_height = (bar.height - self.padding.top - self.padding.bottom).max(0.0);

        let input = Widget::layout(
            &mut self.input,
            &mut tree.children[0],
            renderer,
            &layout::Limits::new(
                Size::new(input_width, 0.0),
                Size::new(input_width, input_height),
            ),
        );

        let input_size = input.size();

        let input = input.move_to(Point::new(
            self.padding.left + AFFORDANCE_WIDTH,
            self.padding.top + (input_height - input_size.height) / 2.0,
        ));

        layout::Node::with_children(bar, vec![input])
    }

    fn semantics(&self, _tree: &Tree, _layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        // Nothing of its own: the inner field already reports
        // `Role::TextField` with its value and actions, and a second node
        // wrapping it would make a screen reader announce the same
        // control twice. See this module's own documentation.
        None
    }

    fn visit_children(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        if let Some(input_layout) = layout.children().next() {
            visit(&mut visit::Widgets::new(
                &mut self.input,
                &mut tree.children[0],
                input_layout,
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
            if let Some(input_layout) = layout.children().next() {
                Widget::operate(
                    &mut self.input,
                    &mut tree.children[0],
                    input_layout,
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

        // The clear affordance is tested before the field, because it
        // sits inside the bar's own bounds and the field would otherwise
        // take the focus click that was meant to empty it.
        if matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                | Event::Touch(touch::Event::FingerPressed { .. })
        ) && let Some(clear) = self.clear_bounds(bounds)
            && cursor.position_over(clear).is_some()
            && let Some(on_clear) = self.on_clear.clone()
        {
            shell.publish(on_clear);
            shell.capture_event();
            return;
        }

        if let Some(input_layout) = layout.children().next() {
            Widget::update(
                &mut self.input,
                &mut tree.children[0],
                event,
                input_layout,
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
        if let Some(clear) = self.clear_bounds(layout.bounds())
            && cursor.is_over(clear)
        {
            return mouse::Interaction::Pointer;
        }

        layout
            .children()
            .next()
            .map(|input_layout| {
                Widget::mouse_interaction(
                    &self.input,
                    &tree.children[0],
                    input_layout,
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
        let appearance = <Theme as Catalog>::style(theme, &self.class);
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
            appearance.background,
        );

        draw_glyph(
            renderer,
            self.glyph_bounds(bounds),
            crate::icon::Icon::SEARCH,
            appearance.glyph_color,
            viewport,
        );

        if let Some(input_layout) = layout.children().next() {
            Widget::draw(
                &self.input,
                &tree.children[0],
                renderer,
                theme,
                style,
                input_layout,
                cursor,
                viewport,
            );
        }

        if let Some(clear) = self.clear_bounds(bounds) {
            draw_glyph(
                renderer,
                clear,
                crate::icon::Icon::CLOSE,
                appearance.glyph_color,
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
        let input_layout = layout.children().next()?;

        Widget::overlay(
            &mut self.input,
            &mut tree.children[0],
            input_layout,
            renderer,
            viewport,
            translation,
        )
    }
}

/// Draws one icon-font glyph, centred in `bounds`.
///
/// These used to be assembled from quads, on the reasoning that "a search
/// bar should not depend on an icon font being installed -- the software
/// fallback and headless tests have no font loader at all". That is not
/// true: `Taim-Icons.ttf` is compiled into the binary and loaded
/// unconditionally by the text stack, and the software renderer draws it
/// exactly as the GPU one does -- which the golden suite now proves on every
/// run.
///
/// It was also not working. The magnifier's lens was a 10px ring that read
/// as a dot, and the clear affordance was a vertical and a horizontal bar,
/// which is a **plus sign** -- an "add" glyph on a control whose job is to
/// erase. Both are now the same glyphs every other control uses.
fn draw_glyph<Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    glyph: crate::icon::Icon,
    color: Color,
    viewport: &Rectangle,
) where
    Renderer: crate::core::text::Renderer,
{
    renderer.fill_text(
        crate::core::text::Text {
            content: glyph.codepoint().to_string(),
            // Same font file as `icon::FONT`, but typed as `Renderer::Font`.
            font: Renderer::ICON_FONT,
            size: crate::core::Pixels(GLYPH_SIZE),
            line_height: crate::core::text::LineHeight::default(),
            bounds: bounds.size(),
            align_x: crate::core::text::Alignment::Center,
            align_y: crate::core::alignment::Vertical::Center,
            shaping: crate::core::text::Shaping::Basic,
            wrapping: crate::core::text::Wrapping::None,
            ellipsis: crate::core::text::Ellipsis::default(),
            hint_factor: None,
        },
        bounds.center(),
        color,
        *viewport,
    );
}

impl<'a, Message, Theme, Renderer> From<SearchBar<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a + Catalog + crate::text_input::Catalog,
    Renderer: 'a + crate::core::text::Renderer,
{
    fn from(search_bar: SearchBar<'a, Message, Theme, Renderer>) -> Self {
        Self::new(search_bar)
    }
}

/// The appearance of a [`SearchBar`]'s chrome.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The bar's fill.
    pub background: Background,
    /// The colour of the magnifier and the clear cross.
    pub glyph_color: Color,
    /// The border around the bar.
    pub border: Border,
}

/// The theme catalog of a [`SearchBar`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> <Self as Catalog>::Class<'a>;

    /// The [`Style`] of a class.
    fn style(&self, class: &<Self as Catalog>::Class<'_>) -> Style;
}

/// A styling function for a [`SearchBar`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> <Self as Catalog>::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &<Self as Catalog>::Class<'_>) -> Style {
        class(self)
    }
}

/// The default styling of a [`SearchBar`] for taim's own theme.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        background: Background::Color(palette.background.weak.color),
        glyph_color: Color {
            a: 0.6,
            ..palette.background.base.text
        },
        border: Border::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar<'a>(value: &str) -> SearchBar<'a, ()> {
        SearchBar::new("Search", value)
    }

    const BOUNDS: Rectangle = Rectangle {
        x: 8.0,
        y: 4.0,
        width: 320.0,
        height: DEFAULT_HEIGHT,
    };

    #[test]
    fn an_empty_field_offers_nothing_to_clear() {
        // Even with a handler: there is no query, so the affordance
        // would be a control that does nothing.
        assert!(!bar("").on_clear(()).has_clear());
        assert!(bar("").on_clear(()).clear_bounds(BOUNDS).is_none());
    }

    #[test]
    fn a_field_with_no_handler_offers_nothing_to_clear() {
        assert!(!bar("rust").has_clear());
        assert!(bar("rust").clear_bounds(BOUNDS).is_none());
    }

    #[test]
    fn a_non_empty_field_with_a_handler_offers_a_clear() {
        assert!(bar("rust").on_clear(()).has_clear());
    }

    #[test]
    fn both_affordances_clear_the_touch_target_floor() {
        let bar = bar("rust").on_clear(());

        let glyph = bar.glyph_bounds(BOUNDS);
        let clear = bar.clear_bounds(BOUNDS).expect("clear region");

        for region in [glyph, clear] {
            assert!(
                region.width >= 44.0 && region.height >= 44.0,
                "affordance {region:?} is below the touch-target floor"
            );
        }
    }

    #[test]
    fn the_affordances_sit_at_opposite_ends_and_do_not_overlap() {
        let bar = bar("rust").on_clear(());

        let glyph = bar.glyph_bounds(BOUNDS);
        let clear = bar.clear_bounds(BOUNDS).expect("clear region");

        assert_eq!(glyph.x, BOUNDS.x);
        assert_eq!(clear.x + clear.width, BOUNDS.x + BOUNDS.width);
        assert!(
            glyph.x + glyph.width <= clear.x,
            "the glyph and the clear region must not overlap"
        );
    }

    #[test]
    fn padding_insets_both_affordances() {
        let bar = bar("rust")
            .on_clear(())
            .padding(Padding::new(0.0).left(12.0).right(20.0));

        assert_eq!(bar.glyph_bounds(BOUNDS).x, BOUNDS.x + 12.0);
        assert_eq!(
            bar.clear_bounds(BOUNDS).expect("clear region").x + AFFORDANCE_WIDTH,
            BOUNDS.x + BOUNDS.width - 20.0
        );
    }
}
