//! A horizontal row of tabs with a sliding selection indicator.
//!
//! # Example
//! ```no_run
//! # mod aura { pub mod widget { pub use aura_widget::*; } pub use aura_widget::Renderer; pub use aura_widget::core::*; }
//! # pub type Element<'a, Message> = aura_widget::core::Element<'a, Message, aura_widget::Theme, aura_widget::Renderer>;
//! use aura::widget::{tabs, tabs::Tab, text};
//!
//! #[derive(Clone)]
//! enum Message {
//!     TabSelected(usize),
//! }
//!
//! # struct State { tab: usize }
//! fn view(state: &State) -> Element<'_, Message> {
//!     tabs([
//!         Tab::new(text("Overview"), "Overview").on_press(Message::TabSelected(0)),
//!         Tab::new(text("Activity"), "Activity").on_press(Message::TabSelected(1)),
//!     ])
//!     .selected(state.tab)
//!     .into()
//! }
//! ```
//!
//! # The indicator slides
//!
//! The selection indicator animates between tabs rather than jumping.
//! That is not decoration: the movement is what tells the user *which*
//! tab the selection came from, which a cut cannot convey. It is driven
//! by an [`Animation<f32>`](aura_core::animation::Animation) over the
//! selected index, so an interrupted change picks up from wherever the
//! indicator currently is instead of restarting.
//!
//! # This is the bar, not the pages
//!
//! [`Tabs`] renders and reports the tab row. The caller swaps the content
//! below it in response to the selection message, exactly as it would for
//! any other control. A swipeable page view that changes the selection by
//! dragging is a separate widget and is not implemented yet.

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

/// The bar's height at rest, sized so each tab clears the 48px
/// touch-target policy.
const DEFAULT_HEIGHT: f32 = 48.0;

/// How thick the sliding indicator is.
const INDICATOR_THICKNESS: f32 = 3.0;

/// A single tab.
#[allow(missing_debug_implementations)]
pub struct Tab<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    content: Element<'a, Message, Theme, Renderer>,
    label: String,
    on_press: Option<Message>,
    /// Pushed down by the bar before layout and draw.
    selected: bool,
}

impl<'a, Message, Theme, Renderer> Tab<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// Creates a tab displaying `content`, announced as `label`.
    ///
    /// The label is the tab's accessible name, and is required for the
    /// same reason [`bottom_nav_bar::Item`](crate::bottom_nav_bar::Item)
    /// requires one: a tab with no name is unusable with a screen
    /// reader, and that should not be reachable by omission.
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

    /// Sets the message produced when this tab is pressed.
    pub fn on_press(mut self, on_press: Message) -> Self {
        self.on_press = Some(on_press);
        self
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Tab<'_, Message, Theme, Renderer>
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
        let class = <Theme as Catalog>::default();
        let appearance = theme.style(&class);

        if let Some(content_layout) = layout.children().next() {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                &renderer::Style {
                    // The indicator alone is not enough: it is a thin
                    // line, and on a dense bar the colour change is what
                    // actually reads at a glance.
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

/// The widget state of a [`Tabs`] bar.
#[derive(Debug)]
pub struct State {
    /// The indicator's position, as a (fractional) tab index.
    indicator: Animation<f32>,
    /// The most recent frame time.
    now: Instant,
}

impl Default for State {
    fn default() -> Self {
        Self {
            indicator: Animation::new(0.0).quick(),
            now: Instant::now(),
        }
    }
}

/// A horizontal row of tabs with a sliding selection indicator.
#[allow(missing_debug_implementations)]
pub struct Tabs<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    tabs: Vec<Tab<'a, Message, Theme, Renderer>>,
    selected: usize,
    height: f32,
    padding: Padding,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> Tabs<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    /// Creates a bar from `tabs`.
    pub fn new(tabs: impl IntoIterator<Item = Tab<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            tabs: tabs.into_iter().collect(),
            selected: 0,
            height: DEFAULT_HEIGHT,
            padding: Padding::ZERO,
            class: Theme::default(),
        }
    }

    /// Sets which tab is selected.
    ///
    /// An out-of-range index selects nothing rather than panicking or
    /// clamping: highlighting the wrong tab is worse than highlighting
    /// none, because it is not visibly wrong.
    pub fn selected(mut self, selected: usize) -> Self {
        self.selected = selected;
        self
    }

    /// Sets the bar's height.
    pub fn height(mut self, height: impl Into<crate::core::Pixels>) -> Self {
        self.height = height.into().0.max(0.0);
        self
    }

    /// Sets the padding around the tabs.
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

    /// Whether the current selection names a tab that exists.
    fn has_selection(&self) -> bool {
        self.selected < self.tabs.len()
    }

    /// Pushes the current selection down into the tabs.
    fn sync_selection(&mut self) {
        for (index, tab) in self.tabs.iter_mut().enumerate() {
            tab.selected = index == self.selected;
        }
    }

    /// The width of one tab cell, given the bar's own width.
    fn cell_width(&self, bar: Size) -> f32 {
        if self.tabs.is_empty() {
            return 0.0;
        }

        let inner = (bar.width - self.padding.left - self.padding.right).max(0.0);

        inner / self.tabs.len() as f32
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Tabs<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: crate::core::Renderer,
    Theme: Catalog,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        // Settled on the initial selection, so a view built with a tab
        // already chosen does not play a slide nobody asked for.
        tree::State::new(State {
            indicator: Animation::new(self.selected as f32).quick(),
            ..State::default()
        })
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children_custom(
            &mut self.tabs,
            |tab_tree, tab| tab.diff(tab_tree),
            |tab| {
                let mut tab_tree = Tree::empty();
                tab_tree.children = vec![Tree::new(tab.content.as_widget())];
                tab_tree
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

        let cell_width = self.cell_width(bar);
        let cell_height = (bar.height - self.padding.top - self.padding.bottom).max(0.0);

        let cell_limits = layout::Limits::new(Size::ZERO, Size::new(cell_width, cell_height));

        let children = self
            .tabs
            .iter_mut()
            .zip(tree.children.iter_mut())
            .enumerate()
            .map(|(index, (tab, tab_tree))| {
                tab.layout(tab_tree, renderer, &cell_limits).move_to(Point::new(
                    self.padding.left + index as f32 * cell_width,
                    self.padding.top,
                ))
            })
            .collect();

        layout::Node::with_children(bar, children)
    }

    fn semantics(&self, tree: &Tree, layout: Layout<'_>) -> Option<crate::core::semantics::Node> {
        use crate::core::semantics::{Actions, Node, Role, State as SemanticState};

        // The bar groups the tabs; the tabs carry the roles and actions.
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

        for ((tab, tab_tree), tab_layout) in self
            .tabs
            .iter_mut()
            .zip(tree.children.iter_mut())
            .zip(layout.children())
        {
            visit(&mut visit::Widgets::new(tab, tab_tree, tab_layout));
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
            for ((tab, tab_tree), tab_layout) in self
                .tabs
                .iter_mut()
                .zip(tree.children.iter_mut())
                .zip(layout.children())
            {
                tab.operate(tab_tree, tab_layout, renderer, operation);
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

            // Only chase a selection that names a real tab; sliding the
            // indicator to a nonexistent index would park it off the end
            // of the bar.
            if self.has_selection() {
                let target = self.selected as f32;

                if (state.indicator.value() - target).abs() > f32::EPSILON {
                    state.indicator.go_mut(target, state.now);
                    shell.request_redraw();
                }
            }

            if let Event::Window(crate::core::window::Event::RedrawRequested(now)) = event {
                state.now = *now;

                if state.indicator.is_animating(*now) {
                    shell.request_redraw();
                }
            }
        }

        for ((tab, tab_tree), tab_layout) in self
            .tabs
            .iter_mut()
            .zip(tree.children.iter_mut())
            .zip(layout.children())
        {
            tab.update(tab_tree, event, tab_layout, cursor, renderer, shell, viewport);

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
        self.tabs
            .iter()
            .zip(tree.children.iter())
            .zip(layout.children())
            .map(|((tab, tab_tree), tab_layout)| {
                tab.mouse_interaction(tab_tree, tab_layout, cursor, viewport, renderer)
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
                border: appearance.border,
                ..renderer::Quad::default()
            },
            appearance.background,
        );

        for ((tab, tab_tree), tab_layout) in self
            .tabs
            .iter()
            .zip(tree.children.iter())
            .zip(layout.children())
        {
            tab.draw(tab_tree, renderer, theme, style, tab_layout, cursor, viewport);
        }

        // The indicator is drawn last and positioned from the animation,
        // not from the selected index, so it lands between two tabs
        // mid-slide.
        if self.has_selection() {
            let state = tree.state.downcast_ref::<State>();
            let position = state.indicator.interpolate(state.now);
            let cell_width = self.cell_width(bounds.size());

            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: bounds.x + self.padding.left + position * cell_width,
                        y: bounds.y + bounds.height - INDICATOR_THICKNESS,
                        width: cell_width,
                        height: INDICATOR_THICKNESS,
                    },
                    border: Border {
                        radius: (INDICATOR_THICKNESS / 2.0).into(),
                        ..Border::default()
                    },
                    ..renderer::Quad::default()
                },
                appearance.indicator,
            );
        }
    }
}

impl<'a, Message, Theme, Renderer> From<Tabs<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a + Catalog,
    Renderer: 'a + crate::core::Renderer,
{
    fn from(tabs: Tabs<'a, Message, Theme, Renderer>) -> Self {
        Self::new(tabs)
    }
}

/// The appearance of a [`Tabs`] bar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The bar's fill.
    pub background: Background,
    /// The colour of an unselected tab's content.
    pub text_color: Color,
    /// The colour of the selected tab's content.
    pub selected_text_color: Color,
    /// The sliding indicator beneath the selected tab.
    pub indicator: Background,
    /// The border around the bar, usually just a bottom edge.
    pub border: Border,
}

/// The theme catalog of a [`Tabs`] bar.
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for a [`Tabs`] bar.
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

/// The default styling of a [`Tabs`] bar for aura's own theme.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        background: Background::Color(palette.background.base.color),
        text_color: Color {
            a: 0.7,
            ..palette.background.base.text
        },
        selected_text_color: palette.primary.base.color,
        indicator: Background::Color(palette.primary.base.color),
        border: Border::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar<'a>(count: usize) -> Tabs<'a, ()> {
        Tabs::new(
            (0..count).map(|index| Tab::new(crate::text::Text::new("t"), format!("Tab {index}"))),
        )
    }

    const BAR: Size = Size {
        width: 600.0,
        height: DEFAULT_HEIGHT,
    };

    #[test]
    fn exactly_one_tab_is_selected() {
        let mut tabs = bar(3).selected(2);
        tabs.sync_selection();

        let selected: Vec<bool> = tabs.tabs.iter().map(|tab| tab.selected).collect();

        assert_eq!(selected, vec![false, false, true]);
    }

    #[test]
    fn an_out_of_range_selection_highlights_nothing() {
        let mut tabs = bar(3).selected(99);
        tabs.sync_selection();

        assert!(!tabs.has_selection());
        assert!(tabs.tabs.iter().all(|tab| !tab.selected));
    }

    #[test]
    fn tabs_divide_the_bar_evenly() {
        assert_eq!(bar(3).cell_width(BAR), 200.0);
        assert_eq!(bar(4).cell_width(BAR), 150.0);
    }

    #[test]
    fn padding_comes_out_of_the_tabs_not_the_bar() {
        let tabs = bar(2).padding(Padding::new(0.0).left(50.0).right(50.0));

        assert_eq!(tabs.cell_width(BAR), 250.0);
    }

    #[test]
    fn an_empty_bar_has_no_cell_width_and_does_not_divide_by_zero() {
        assert_eq!(bar(0).cell_width(BAR), 0.0);
    }

    #[test]
    fn the_indicator_starts_settled_on_the_initial_selection() {
        // A view built with tab 2 already chosen must not animate in from
        // tab 0 on its first frame.
        let tabs = bar(3).selected(2);
        let state = tabs.state();
        let state = state.downcast_ref::<State>();

        assert_eq!(state.indicator.value(), 2.0);
        assert!(!state.indicator.is_animating(Instant::now()));
    }
}
