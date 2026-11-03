//! Choice, list and window-chrome views.
use super::look::{self, Gesture, Label};
use super::{Align, Icon, button, column, container, icon, row, space, text};
use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::{Axis, Constraints, Sizing};
use crate::state::{IntoProp, Prop, State};
use crate::{AnyView, Border, Child, Color, Event, Handled, Point, Rectangle, Size, View};
use crate::{mouse, semantics};

use std::cell::Cell;
use taim_core::widget;

/// One option of a group: chosen while `selected` equals `value`;
/// clicking it sets `selected` to `value`.
pub fn radio<T>(label: impl Into<String>, value: T, selected: State<T>) -> Radio<T>
where
    T: Clone + PartialEq + Send + Sync + 'static,
{
    Radio {
        label: Label::new(label, None),
        value,
        selected,
        shown: selected.into_prop(),
        pressed: false,
        id: widget::Id::unique(),
    }
}

/// See [`radio`].
pub struct Radio<T> {
    label: Label,
    value: T,
    selected: State<T>,
    shown: Prop<T>,
    pressed: bool,
    id: widget::Id,
}

const DOT: f32 = 18.0;

impl<T: Clone + PartialEq + Send + Sync + 'static> View for Radio<T> {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _constraints: Constraints) -> Size {
        let label = self.label.measure(cx);
        Size::new(DOT + 8.0 + label.width, label.height.max(DOT))
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let bounds = cx.bounds();
        let palette = *cx.theme().palette();
        let on = self.shown.get() == self.value;
        let ring = Rectangle::new(
            Point::new(bounds.x, bounds.y + (bounds.height - DOT) / 2.0),
            Size::new(DOT, DOT),
        );
        let edge = if on {
            palette.primary.base.color
        } else {
            palette.background.strong.color
        };

        cx.fill(
            ring,
            palette.background.base.color,
            Border::default().width(1.5).color(edge).rounded(DOT / 2.0),
        );
        if on {
            cx.fill(
                ring.shrink(5.0),
                palette.primary.base.color,
                Border::default().rounded(DOT / 2.0),
            );
        }
        self.label.paint(
            cx,
            Point::new(
                bounds.x + DOT + 8.0,
                bounds.y + (bounds.height - self.label.size().height) / 2.0,
            ),
            cx.text_color(),
        );
        if cx.has_focus() {
            look::focus_ring(cx, ring, DOT / 2.0);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match look::gesture(event, cx.pointer()) {
            Some(Gesture::Down(at)) if cx.bounds().contains(at) => {
                self.pressed = true;
                cx.request_focus();
                Handled::Yes
            }
            Some(Gesture::Up(at)) if self.pressed => {
                self.pressed = false;
                if at.is_some_and(|at| cx.bounds().contains(at)) {
                    let _ = self.selected.set(self.value.clone());
                }
                Handled::Yes
            }
            _ if cx.has_focus() && look::is_activation(event) => {
                let _ = self.selected.set(self.value.clone());
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        let mut state = look::interactive_state(true, cx.has_focus());
        state.set(semantics::State::CHECKED, self.shown.get() == self.value);
        Some(
            semantics::Node::new(self.id.clone(), cx.bounds(), semantics::Role::RadioButton)
                .with_name(self.label.text())
                .with_state(state)
                .with_actions(semantics::Actions::ACTIVATE),
        )
    }

    fn cursor(&self) -> Option<mouse::Interaction> {
        Some(mouse::Interaction::Pointer)
    }

    fn focusable(&self) -> bool {
        true
    }
}

/// A button showing the chosen option; clicking it opens the list of
/// `options` below it. Bound to `selected` (an index into `options`).
pub fn dropdown<S: Into<String>>(
    options: impl IntoIterator<Item = S>,
    selected: State<Option<usize>>,
) -> Dropdown {
    Dropdown {
        options: options
            .into_iter()
            .map(|option| Label::new(option, None))
            .collect(),
        placeholder: Label::new("Choose…", None),
        chevrons: [
            Label::glyph(Icon::CHEVRON_DOWN, 18.0),
            Label::glyph(Icon::CHEVRON_UP, 18.0),
        ],
        selected,
        shown: selected.into_prop(),
        open: false,
        hovered: Cell::new(None),
        id: widget::Id::unique(),
    }
}

/// See [`dropdown`].
pub struct Dropdown {
    options: Vec<Label>,
    placeholder: Label,
    chevrons: [Label; 2],
    selected: State<Option<usize>>,
    shown: Prop<Option<usize>>,
    open: bool,
    hovered: Cell<Option<usize>>,
    id: widget::Id,
}

const ROW: f32 = 32.0;

impl Dropdown {
    /// Text shown while nothing is chosen.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Label::new(placeholder, None);
        self
    }

    fn menu(&self, bounds: Rectangle) -> Rectangle {
        Rectangle::new(
            Point::new(bounds.x, bounds.y + bounds.height + 4.0),
            Size::new(bounds.width, ROW * self.options.len() as f32),
        )
    }

    fn option_at(&self, bounds: Rectangle, at: Point) -> Option<usize> {
        let menu = self.menu(bounds);
        menu.contains(at)
            .then(|| ((at.y - menu.y) / ROW) as usize)
            .filter(|index| *index < self.options.len())
    }
}

impl View for Dropdown {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _constraints: Constraints) -> Size {
        let mut widest = self.placeholder.measure(cx).width;
        for option in &mut self.options {
            widest = widest.max(option.measure(cx).width);
        }
        for chevron in &mut self.chevrons {
            let _ = chevron.measure(cx);
        }
        Size::new(widest + 48.0, ROW + 4.0)
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let bounds = cx.bounds();
        let palette = *cx.theme().palette();
        let edge = if self.open || cx.has_focus() {
            palette.primary.base.color
        } else {
            palette.background.strong.color
        };
        cx.fill(
            bounds,
            palette.background.base.color,
            Border::default().width(1.0).color(edge).rounded(6.0),
        );

        let at = Point::new(bounds.x + 12.0, bounds.y + 8.0);
        match self.shown.get().and_then(|index| self.options.get(index)) {
            Some(option) => option.paint(cx, at, cx.text_color()),
            None => self
                .placeholder
                .paint(cx, at, cx.muted_on(palette.background.weak.color)),
        }

        let arrow = Rectangle::new(
            Point::new(bounds.x + bounds.width - 32.0, bounds.y),
            Size::new(24.0, bounds.height),
        );
        let chevron = if self.open {
            &self.chevrons[1]
        } else {
            &self.chevrons[0]
        };
        chevron.paint_centered(cx, arrow, cx.quiet_icon_on(palette.background.weak.color));
    }

    fn paint_over(&self, cx: &mut PaintCx<'_>) {
        if !self.open {
            return;
        }
        let bounds = cx.bounds();
        let palette = *cx.theme().palette();
        let menu = self.menu(bounds);
        cx.fill(
            menu.expand(4.0),
            palette.background.base.color,
            Border::default()
                .width(1.0)
                .color(palette.background.strong.color)
                .rounded(8.0),
        );

        let chosen = self.shown.get();
        for (index, option) in self.options.iter().enumerate() {
            let row = Rectangle::new(
                Point::new(menu.x, menu.y + ROW * index as f32),
                Size::new(menu.width, ROW),
            );
            if self.hovered.get() == Some(index) || chosen == Some(index) {
                let tint = if chosen == Some(index) {
                    palette.primary.weak.color
                } else {
                    palette.background.weak.color
                };
                cx.fill(row, tint, Border::default().rounded(4.0));
            }
            option.paint(cx, Point::new(row.x + 12.0, row.y + 7.0), cx.text_color());
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let bounds = cx.bounds();

        match look::gesture(event, cx.pointer()) {
            Some(Gesture::Down(at)) if bounds.contains(at) => {
                self.open = !self.open;
                cx.request_focus();
                cx.request_paint();
                Handled::Yes
            }
            Some(Gesture::Down(at)) if self.open => {
                if let Some(index) = self.option_at(bounds, at) {
                    let _ = self.selected.set(Some(index));
                }
                self.open = false;
                cx.request_paint();
                Handled::Yes
            }
            Some(Gesture::Move(at)) if self.open => {
                let hovered = self.option_at(bounds, at);
                if hovered != self.hovered.get() {
                    self.hovered.set(hovered);
                    cx.request_paint();
                }
                Handled::from(self.menu(bounds).contains(at))
            }
            _ if cx.has_focus() => {
                use crate::keyboard::{self, key::Named};
                let Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(named),
                    ..
                }) = event
                else {
                    return Handled::No;
                };
                let last = self.options.len().saturating_sub(1);
                let current = self.selected.get();
                let next = match named {
                    Named::ArrowDown => Some(current.map_or(0, |index| (index + 1).min(last))),
                    Named::ArrowUp => Some(current.map_or(0, |index| index.saturating_sub(1))),
                    Named::Escape => {
                        self.open = false;
                        cx.request_paint();
                        return Handled::Yes;
                    }
                    _ => return Handled::No,
                };
                let _ = self.selected.set(next);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        let mut state = look::interactive_state(true, cx.has_focus());
        state.set(semantics::State::EXPANDED, self.open);
        let value = self
            .shown
            .get()
            .and_then(|index| self.options.get(index))
            .map_or_else(String::new, Label::text);
        Some(
            semantics::Node::new(self.id.clone(), cx.bounds(), semantics::Role::Button)
                .with_name(value)
                .with_state(state)
                .with_actions(semantics::Actions::ACTIVATE | semantics::Actions::TOGGLE_EXPANDED),
        )
    }

    fn cursor(&self) -> Option<mouse::Interaction> {
        Some(mouse::Interaction::Pointer)
    }

    fn focusable(&self) -> bool {
        true
    }

    fn is_layer(&self) -> bool {
        // The open list must cover whatever is painted after the button.
        self.open
    }
}

/// A modal dialog: a card centered over a scrim while `open` is true.
/// Tapping the scrim closes it.
pub fn dialog(open: State<bool>, content: impl Into<AnyView>) -> Dialog {
    Dialog {
        open,
        shown: open.into_prop(),
        content: Child::new(container(content).padding(24.0)),
        card: Cell::new(Rectangle::default()),
    }
}

/// See [`dialog`].
pub struct Dialog {
    open: State<bool>,
    shown: Prop<bool>,
    content: Child,
    card: Cell<Rectangle>,
}

impl View for Dialog {
    fn sizing(&self) -> Sizing {
        Sizing::FILL
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let screen = constraints.with_sizing(Sizing::FILL).max;
        let room = Size::new((screen.width - 64.0).min(520.0), screen.height - 64.0);
        let card = self.content.measure(cx, Constraints::loose(room));
        let origin = Point::new(
            (screen.width - card.width) / 2.0,
            (screen.height - card.height) / 2.0,
        );
        self.content.place(origin);
        self.card.set(Rectangle::new(origin, card));
        screen
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        if !self.shown.get() {
            return;
        }
        let bounds = cx.bounds();
        let palette = *cx.theme().palette();
        cx.fill(bounds, Color::BLACK.scale_alpha(0.45), Border::default());

        let local = self.card.get();
        let card = Rectangle::new(
            Point::new(bounds.x + local.x, bounds.y + local.y),
            local.size(),
        );
        // The card is liquid glass over the dimmed page.
        let _ = palette;
        super::glass::pane(
            cx,
            card,
            22.0,
            taim_core::material::Material::LIQUID,
            None,
            2.0,
        );
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        if !self.shown.get() {
            return Handled::No;
        }
        let bounds = cx.bounds();
        let local = self.card.get();
        let card = Rectangle::new(
            Point::new(bounds.x + local.x, bounds.y + local.y),
            local.size(),
        );

        if look::is_back(event) {
            let _ = self.open.set(false);
            return Handled::Yes;
        }

        match look::gesture(event, cx.pointer()) {
            Some(Gesture::Down(at)) if !card.contains(at) => {
                let _ = self.open.set(false);
                Handled::Yes
            }
            _ => Handled::from(look::is_pointer(event)),
        }
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        self.shown.get().then(|| {
            semantics::Node::new(widget::Id::unique(), cx.bounds(), semantics::Role::Window)
                .with_actions(semantics::Actions::DISMISS)
        })
    }

    fn is_layer(&self) -> bool {
        true
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        if self.shown.get() {
            visit(&mut self.content);
        }
    }
}

/// A square button showing just an icon.
pub fn icon_button(glyph: Icon) -> super::Button {
    button(String::new()).label_view(icon(glyph).size(18))
}

/// A list row: optional leading icon, a title and an optional subtitle.
pub fn list_tile(
    leading: Option<Icon>,
    title: impl IntoProp<String>,
    subtitle: Option<String>,
) -> AnyView {
    let mut lines = vec![AnyView::from(text(title).size(15))];
    if let Some(subtitle) = subtitle {
        lines.push(text(subtitle).size(12).into());
    }

    let mut parts = Vec::new();
    if let Some(glyph) = leading {
        parts.push(AnyView::from(icon(glyph).size(20)));
    }
    parts.push(column(lines).gap(2.0).into());

    container(row(parts).gap(16.0).align(Align::Center).fill_width())
        .padding([10.0, 16.0])
        .fill_width()
        .into()
}

/// A top bar: a title and trailing actions.
pub fn app_bar(
    title: impl IntoProp<String>,
    actions: impl IntoIterator<Item = AnyView>,
) -> AnyView {
    let mut parts = vec![AnyView::from(text(title).size(20)), space().into()];
    parts.extend(actions);

    container(row(parts).gap(8.0).align(Align::Center).fill_width())
        .padding([12.0, 16.0])
        .fill_width()
        .into()
}

/// Children flowed into rows of `columns` equal cells.
pub fn grid(columns: usize, children: impl IntoIterator<Item = AnyView>) -> Grid {
    Grid {
        columns: columns.max(1),
        children: children.into_iter().map(Child::new).collect(),
        gap: 8.0,
    }
}

/// See [`grid`].
pub struct Grid {
    columns: usize,
    children: Vec<Child>,
    gap: f32,
}

impl Grid {
    /// Space between cells.
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }
}

impl View for Grid {
    fn sizing(&self) -> Sizing {
        Sizing {
            width: Axis::Fill(1),
            height: Axis::Hug,
        }
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let width = constraints.with_sizing(self.sizing()).max.width;
        let width = if width.is_finite() {
            width
        } else {
            120.0 * self.columns as f32
        };
        let cell = (width - self.gap * (self.columns - 1) as f32) / self.columns as f32;
        let mut y = 0.0;

        for row in self.children.chunks_mut(self.columns) {
            let mut tallest: f32 = 0.0;
            for (column, child) in row.iter_mut().enumerate() {
                let size = child.measure(cx, Constraints::loose(Size::new(cell, f32::INFINITY)));
                child.place(Point::new(column as f32 * (cell + self.gap), y));
                tallest = tallest.max(size.height);
            }
            y += tallest + self.gap;
        }

        Size::new(width, (y - self.gap).max(0.0))
    }

    fn paint(&self, _cx: &mut PaintCx<'_>) {}

    fn describe(&self, _cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        for child in &mut self.children {
            visit(child);
        }
    }

    fn children_rev(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        for child in self.children.iter_mut().rev() {
            visit(child);
        }
    }
}
