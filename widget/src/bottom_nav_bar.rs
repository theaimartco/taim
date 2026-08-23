//! A tab bar pinned to the bottom of the screen.
//!
//! # Example
//! ```no_run
//! # mod aura { pub mod widget { pub use aura_widget::*; } pub use aura_widget::Renderer; pub use aura_widget::core::*; }
//! # pub type Element<'a, Message> = aura_widget::core::Element<'a, Message, aura_widget::Theme, aura_widget::Renderer>;
//! use aura::widget::{bottom_nav_bar, bottom_nav_bar::Item, text};
//!
//! #[derive(Clone)]
//! enum Message {
//!     TabSelected(usize),
//! }
//!
//! # struct State { tab: usize }
//! fn view(state: &State) -> Element<'_, Message> {
//!     bottom_nav_bar([
//!         Item::new(text("H"), "Home").on_press(Message::TabSelected(0)),
//!         Item::new(text("S"), "Search").on_press(Message::TabSelected(1)),
//!     ])
//!     .selected(state.tab)
//!     .into()
//! }
//! ```
//!
//! # Selection is visible *and* reported
//!
//! Each [`Item`] is a widget in its own right, reporting
//! [`Role::Tab`](aura_core::semantics::Role::Tab) with the
//! [`SELECTED`](aura_core::semantics::State::SELECTED) flag, so the
//! selected tab is announced as selected rather than merely tinted. The
//! bar itself is a [`Role::Group`](aura_core::semantics::Role::Group)
//! that owns the indicator and the equal-width layout -- the same split
//! `app_bar` uses, where the container is a container and the controls
//! inside it carry the roles and the touch targets.

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

/// The bar's height at rest. Chosen so each item clears the 48px
/// touch-target policy with room for the indicator above it.
const DEFAULT_HEIGHT: f32 = 64.0;

/// The selection indicator's dimensions, as a pill behind the selected
/// item's content.
const INDICATOR_HEIGHT: f32 = 32.0;
const INDICATOR_MAX_WIDTH: f32 = 64.0;

/// An item in a [`BottomNavigationBar`].
///
/// The item owns its own press handling, selected styling, and
/// `Role::Tab` semantics; the bar only decides where it sits and which
/// one is selected.
#[allow(missing_debug_implementations)]
pub struct Item<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    content: Element<'a, Message, Theme, Renderer>,
    label: String,
    on_press: Option<Message>,
    /// Set by the bar before layout and draw. An item does not know its
    /// own index, so selection is pushed down rather than looked up.
    selected: bool,
}

impl<'a, Message, Theme, Renderer> Item<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// Creates an item displaying `content`, announced as `label`.
    ///
    /// The label is the item's accessible name. It is required rather
    /// than optional because an icon-only tab with no name is unusable
    /// with a screen reader, and that is not a decision a caller should
    /// be able to make by omission.
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

    /// Sets the message produced when this item is pressed.
    pub fn on_press(mut self, on_press: Message) -> Self {
        self.on_press = Some(on_press);
        self
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Item<'_, Message, Theme, Renderer>
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

        // Centred in its cell: a tab's content is an icon over a label,
        // and off-centre tabs read as a layout bug at a glance.
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
            flags |= SemanticState::SELECTED;
        }

        Some(
            Node::new(tree.id().clone(), layout.bounds(), Role::Tab)
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
        if self.on_press.is_some() && cursor.is_over(layout.bounds()) {
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
        // Items are not styled individually: a tab bar whose tabs
        // disagreed about their own appearance would not read as one
        // control, so an item resolves the bar's default class.
        let class = <Theme as Catalog>::default();
        let appearance = theme.style(&class);
        let bounds = layout.bounds();

        // The indicator: the selected tab's pill. This is the difference
        // the previous implementation computed and then threw away, which
        // left every tab looking identical.
        if self.selected {
            let width = bounds.width.min(INDICATOR_MAX_WIDTH);

            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: bounds.x + (bounds.width - width) / 2.0,
                        y: bounds.y + (bounds.height - INDICATOR_HEIGHT) / 2.0,
                        width,
                        height: INDICATOR_HEIGHT,
                    },
                    border: Border {
                        radius: (INDICATOR_HEIGHT / 2.0).into(),
                        ..Border::default()
                    },
                    ..renderer::Quad::default()
                },
                appearance.indicator,
            );
        }

        if let Some(content_layout) = layout.children().next() {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                &renderer::Style {
                    // Selected and unselected content differ in colour as
                    // well as in the indicator behind them: colour alone
                    // is not an accessible distinction, and the indicator
                    // alone is easy to miss on a small screen.
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

/// A tab bar pinned to the bottom of the screen.
#[allow(missing_debug_implementations)]
pub struct BottomNavigationBar<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    items: Vec<Item<'a, Message, Theme, Renderer>>,
    selected: usize,
    height: f32,
    padding: Padding,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> BottomNavigationBar<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// Creates a bar from `items`.
    pub fn new(items: impl IntoIterator<Item = Item<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            items: items.into_iter().collect(),
            selected: 0,
            height: DEFAULT_HEIGHT,
            padding: Padding::ZERO,
            class: Theme::default(),
        }
    }

    /// Sets which item is selected.
    ///
    /// An out-of-range index selects nothing rather than panicking: a
    /// stale index from application state should not take the window
    /// down, and "no tab highlighted" is a legible failure.
    pub fn selected(mut self, selected: usize) -> Self {
        self.selected = selected;
        self
    }

    /// Sets the bar's height.
    pub fn height(mut self, height: impl Into<crate::core::Pixels>) -> Self {
        self.height = height.into().0.max(0.0);
        self
    }

    /// Sets the padding around the items.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the style of the bar.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Pushes the current selection down into the items.
    ///
    /// Called at the start of layout and draw so an item always knows
    /// whether it is the selected one without having to be told its own
    /// index.
    fn sync_selection(&mut self) {
        for (index, item) in self.items.iter_mut().enumerate() {
            item.selected = index == self.selected;
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for BottomNavigationBar<'_, Message, Theme, Renderer>
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
        // Items are not `Element`s, so the plain `diff_children` does not
        // apply: each item keeps a tree whose single child is the item's
        // own content.
        tree.diff_children_custom(
            &mut self.items,
            |item_tree, item| item.diff(item_tree),
            |item| {
                let mut item_tree = Tree::empty();
                item_tree.children = vec![Tree::new(item.content.as_widget())];
                item_tree
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

        let bar = limits.resolve(
            Length::Fill,
            Length::Fixed(self.height),
            Size::new(0.0, self.height),
        );

        let inner = Size::new(
            (bar.width - self.padding.left - self.padding.right).max(0.0),
            (bar.height - self.padding.top - self.padding.bottom).max(0.0),
        );

        // Equal-width cells: a tab bar whose tabs were sized by their
        // labels would move under the user as the selection changed.
        let cell_width = if self.items.is_empty() {
            0.0
        } else {
            inner.width / self.items.len() as f32
        };

        let cell_limits = layout::Limits::new(Size::ZERO, Size::new(cell_width, inner.height));

        let children = self
            .items
            .iter_mut()
            .zip(tree.children.iter_mut())
            .enumerate()
            .map(|(index, (item, item_tree))| {
                item.layout(item_tree, renderer, &cell_limits)
                    .move_to(Point::new(
                        self.padding.left + index as f32 * cell_width,
                        self.padding.top,
                    ))
            })
            .collect();

        layout::Node::with_children(bar, children)
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Node, Role, State as SemanticState};

        // The bar is a container: its tabs carry the roles, the actions,
        // and the touch targets.
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
        self.sync_selection();

        for ((item, item_tree), item_layout) in self
            .items
            .iter_mut()
            .zip(tree.children.iter_mut())
            .zip(layout.children())
        {
            visit(&mut visit::Widgets::new(item, item_tree, item_layout));
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
            for ((item, item_tree), item_layout) in self
                .items
                .iter_mut()
                .zip(tree.children.iter_mut())
                .zip(layout.children())
            {
                item.operate(item_tree, item_layout, renderer, operation);
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

        for ((item, item_tree), item_layout) in self
            .items
            .iter_mut()
            .zip(tree.children.iter_mut())
            .zip(layout.children())
        {
            item.update(
                item_tree,
                event,
                item_layout,
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
        self.items
            .iter()
            .zip(tree.children.iter())
            .zip(layout.children())
            .map(|((item, item_tree), item_layout)| {
                item.mouse_interaction(item_tree, item_layout, cursor, viewport, renderer)
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

        renderer.fill_quad(
            renderer::Quad {
                bounds: layout.bounds(),
                border: appearance.border,
                ..renderer::Quad::default()
            },
            appearance.background,
        );

        for ((item, item_tree), item_layout) in self
            .items
            .iter()
            .zip(tree.children.iter())
            .zip(layout.children())
        {
            item.draw(
                item_tree,
                renderer,
                theme,
                style,
                item_layout,
                cursor,
                viewport,
            );
        }
    }
}

impl<'a, Message, Theme, Renderer> From<BottomNavigationBar<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a + Catalog,
    Renderer: 'a + crate::core::Renderer,
{
    fn from(bar: BottomNavigationBar<'a, Message, Theme, Renderer>) -> Self {
        Self::new(bar)
    }
}

/// The appearance of a [`BottomNavigationBar`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The bar's fill.
    pub background: Background,
    /// The colour of an unselected item's content.
    pub text_color: Color,
    /// The colour of the selected item's content.
    pub selected_text_color: Color,
    /// The pill drawn behind the selected item.
    pub indicator: Background,
    /// The border around the bar, usually just a top edge.
    pub border: Border,
}

/// The theme catalog of a [`BottomNavigationBar`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for a [`BottomNavigationBar`].
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

/// The default styling of a [`BottomNavigationBar`] for aura's own theme.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        background: Background::Color(palette.background.weak.color),
        text_color: Color {
            a: 0.7,
            ..palette.background.base.text
        },
        selected_text_color: palette.primary.base.text,
        indicator: Background::Color(palette.primary.base.color),
        border: Border::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar<'a>(count: usize) -> BottomNavigationBar<'a, ()> {
        BottomNavigationBar::new(
            (0..count).map(|index| Item::new(crate::text::Text::new("i"), format!("Tab {index}"))),
        )
    }

    #[test]
    fn exactly_one_item_is_selected() {
        let mut bar = bar(3).selected(1);
        bar.sync_selection();

        let selected: Vec<bool> = bar.items.iter().map(|item| item.selected).collect();

        assert_eq!(selected, vec![false, true, false]);
    }

    #[test]
    fn an_out_of_range_selection_highlights_nothing() {
        // Regression guard: the previous implementation computed
        // selection and discarded it, so every tab rendered identically.
        // Clamping to the last item instead would silently highlight the
        // wrong tab, which is worse than highlighting none.
        let mut bar = bar(3).selected(99);
        bar.sync_selection();

        assert!(bar.items.iter().all(|item| !item.selected));
    }

    #[test]
    fn selection_reaches_the_items_that_render_it() {
        let mut bar = bar(2).selected(0);
        bar.sync_selection();
        assert!(bar.items[0].selected);

        bar.selected = 1;
        bar.sync_selection();

        assert!(!bar.items[0].selected, "the old selection must be cleared");
        assert!(bar.items[1].selected);
    }
}
