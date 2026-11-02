//! Views that reveal, step through or page their content.
use super::look::{self, Gesture, Label, Tween};
use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::{Axis, Constraints, Sizing};
use crate::state::{IntoProp, Prop, State};
use crate::{
    AnyView, Border, Child, Event, Handled, Instant, Point, Rectangle, Size, Vector, View,
};
use crate::{mouse, semantics};

use std::cell::Cell;
use std::ops::RangeInclusive;
use taim_core::widget;

/// − value + buttons that move `value` within `range`.
pub fn stepper(value: State<i32>, range: RangeInclusive<i32>) -> Stepper {
    Stepper {
        value,
        shown: value.into_prop(),
        range,
        minus: Label::new("−", Some(18.0)),
        plus: Label::new("+", Some(18.0)),
        number: Label::new("", None),
        pressed: None,
        id: widget::Id::unique(),
    }
}

/// See [`stepper`].
pub struct Stepper {
    value: State<i32>,
    shown: Prop<i32>,
    range: RangeInclusive<i32>,
    minus: Label,
    plus: Label,
    number: Label,
    pressed: Option<bool>,
    id: widget::Id,
}

const STEP_BUTTON: f32 = 32.0;

impl Stepper {
    fn halves(bounds: Rectangle) -> (Rectangle, Rectangle) {
        let side = Size::new(STEP_BUTTON, bounds.height);
        (
            Rectangle::new(bounds.position(), side),
            Rectangle::new(
                Point::new(bounds.x + bounds.width - STEP_BUTTON, bounds.y),
                side,
            ),
        )
    }

    fn step(&self, up: bool) {
        let next = self.value.get() + if up { 1 } else { -1 };
        let _ = self
            .value
            .set(next.clamp(*self.range.start(), *self.range.end()));
    }
}

impl View for Stepper {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _constraints: Constraints) -> Size {
        self.number.set(&self.shown.get().to_string());
        let number = self.number.measure(cx);
        let _ = self.minus.measure(cx);
        let _ = self.plus.measure(cx);
        Size::new(
            STEP_BUTTON * 2.0 + number.width.max(24.0) + 16.0,
            STEP_BUTTON,
        )
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let bounds = cx.bounds();
        let palette = *cx.theme().palette();
        let value = self.shown.get();
        let (minus, plus) = Self::halves(bounds);

        cx.fill(
            bounds,
            palette.background.weak.color,
            Border::default()
                .width(1.0)
                .color(palette.background.strong.color)
                .rounded(8.0),
        );

        let ink = |enabled: bool| {
            if enabled {
                palette.primary.base.color
            } else {
                palette.background.strong.color
            }
        };
        self.minus
            .paint_centered(cx, minus, ink(value > *self.range.start()));
        self.plus
            .paint_centered(cx, plus, ink(value < *self.range.end()));
        self.number.paint_centered(cx, bounds, cx.text_color());

        if cx.has_focus() {
            look::focus_ring(cx, bounds, 8.0);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let (minus, plus) = Self::halves(cx.bounds());

        match look::gesture(event, cx.pointer()) {
            Some(Gesture::Down(at)) if minus.contains(at) || plus.contains(at) => {
                self.pressed = Some(plus.contains(at));
                cx.request_focus();
                Handled::Yes
            }
            Some(Gesture::Up(at)) => {
                let Some(up) = self.pressed.take() else {
                    return Handled::No;
                };
                let target = if up { plus } else { minus };
                if at.is_some_and(|at| target.contains(at)) {
                    self.step(up);
                }
                Handled::Yes
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
                match named {
                    Named::ArrowUp | Named::ArrowRight => self.step(true),
                    Named::ArrowDown | Named::ArrowLeft => self.step(false),
                    _ => return Handled::No,
                }
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        Some(
            semantics::Node::new(self.id.clone(), cx.bounds(), semantics::Role::Slider)
                .with_state(look::interactive_state(true, cx.has_focus()))
                .with_value(semantics::Value::Number {
                    current: f64::from(self.shown.get()),
                    min: f64::from(*self.range.start()),
                    max: f64::from(*self.range.end()),
                }),
        )
    }

    fn cursor(&self) -> Option<mouse::Interaction> {
        Some(mouse::Interaction::Pointer)
    }

    fn focusable(&self) -> bool {
        true
    }
}

/// `content` with `tip` shown above it while the pointer rests on it.
pub fn tooltip(content: impl Into<AnyView>, tip: impl Into<String>) -> Tooltip {
    Tooltip {
        content: Child::new(content.into()),
        tip: Label::new(tip, Some(12.0)),
        hovered: false,
    }
}

/// See [`tooltip`].
pub struct Tooltip {
    content: Child,
    tip: Label,
    hovered: bool,
}

impl View for Tooltip {
    fn sizing(&self) -> Sizing {
        self.content.sizing()
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let _ = self.tip.measure(cx);
        let size = self.content.measure(cx, constraints);
        self.content.place(Point::ORIGIN);
        size
    }

    fn paint(&self, _cx: &mut PaintCx<'_>) {}

    fn paint_over(&self, cx: &mut PaintCx<'_>) {
        if !self.hovered {
            return;
        }
        let bounds = cx.bounds();
        let size = self.tip_size();
        let bubble = Rectangle::new(
            Point::new(
                bounds.center().x - size.width / 2.0,
                bounds.y - size.height - 6.0,
            ),
            size,
        );
        let palette = *cx.theme().palette();
        cx.fill(
            bubble,
            palette.background.strongest.color,
            Border::default().rounded(6.0),
        );
        self.tip
            .paint_centered(cx, bubble, palette.background.base.color);
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        if matches!(
            event,
            Event::Mouse(mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft)
        ) {
            let hovered = cx.is_hovered();
            if hovered != self.hovered {
                self.hovered = hovered;
                cx.request_paint();
            }
        }
        Handled::No
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        Some(
            semantics::Node::new(widget::Id::unique(), cx.bounds(), semantics::Role::Group)
                .with_description(self.tip.text()),
        )
    }

    fn is_layer(&self) -> bool {
        // The bubble must be above whatever is painted after the content.
        true
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        visit(&mut self.content);
    }
}

impl Tooltip {
    fn tip_size(&self) -> Size {
        // Label measured in `measure`; pad it.
        let text = self.tip.size();
        Size::new(text.width + 16.0, text.height + 10.0)
    }
}

/// Collapsible sections, at most one open at a time, bound to `open`.
pub fn accordion<T: Into<String>>(
    sections: impl IntoIterator<Item = (T, AnyView)>,
    open: State<Option<usize>>,
) -> Accordion {
    Accordion {
        sections: sections
            .into_iter()
            .map(|(title, body)| Section {
                title: Label::new(title, Some(15.0)),
                chevron: Label::new("›", Some(18.0)),
                body: Child::new(body),
                header: Cell::new(Rectangle::default()),
            })
            .collect(),
        open,
        shown: open.into_prop(),
        pressed: None,
        id: widget::Id::unique(),
    }
}

struct Section {
    title: Label,
    chevron: Label,
    body: Child,
    header: Cell<Rectangle>,
}

/// See [`accordion`].
pub struct Accordion {
    sections: Vec<Section>,
    open: State<Option<usize>>,
    shown: Prop<Option<usize>>,
    pressed: Option<usize>,
    id: widget::Id,
}

const HEADER: f32 = 44.0;

impl View for Accordion {
    fn sizing(&self) -> Sizing {
        Sizing {
            width: Axis::Fill(1),
            height: Axis::Hug,
        }
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let open = self.shown.get();
        let width = constraints.with_sizing(self.sizing()).max.width;
        let width = if width.is_finite() { width } else { 320.0 };
        let mut y = 0.0;

        for (index, section) in self.sections.iter_mut().enumerate() {
            let _ = section.title.measure(cx);
            let _ = section.chevron.measure(cx);
            section
                .header
                .set(Rectangle::new(Point::new(0.0, y), Size::new(width, HEADER)));
            y += HEADER;

            if open == Some(index) {
                let room = Size::new(width - 32.0, f32::INFINITY);
                let body = section.body.measure(cx, Constraints::loose(room));
                section.body.place(Point::new(16.0, y + 4.0));
                y += body.height + 12.0;
            }
        }

        Size::new(width, y)
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let bounds = cx.bounds();
        let palette = *cx.theme().palette();
        let open = self.shown.get();

        for (index, section) in self.sections.iter().enumerate() {
            let local = section.header.get();
            let header = Rectangle::new(
                Point::new(bounds.x + local.x, bounds.y + local.y),
                local.size(),
            );
            if index > 0 {
                let line = Rectangle::new(header.position(), Size::new(header.width, 1.0));
                cx.fill(line, palette.background.strong.color, Border::default());
            }
            section.title.paint(
                cx,
                Point::new(header.x + 12.0, header.y + 12.0),
                cx.text_color(),
            );
            let chevron = Rectangle::new(
                Point::new(header.x + header.width - 36.0, header.y),
                Size::new(24.0, HEADER),
            );
            let tint = if open == Some(index) {
                palette.primary.base.color
            } else {
                palette.background.strongest.color
            };
            section.chevron.paint_centered(cx, chevron, tint);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let bounds = cx.bounds();
        let hit = |at: Point, sections: &[Section]| {
            sections.iter().position(|section| {
                let local = section.header.get();
                Rectangle::new(
                    Point::new(bounds.x + local.x, bounds.y + local.y),
                    local.size(),
                )
                .contains(at)
            })
        };

        match look::gesture(event, cx.pointer()) {
            Some(Gesture::Down(at)) => {
                self.pressed = hit(at, &self.sections);
                Handled::from(self.pressed.is_some())
            }
            Some(Gesture::Up(at)) => {
                let Some(pressed) = self.pressed.take() else {
                    return Handled::No;
                };
                if at.and_then(|at| hit(at, &self.sections)) == Some(pressed) {
                    self.open.update(|open| {
                        *open = if *open == Some(pressed) {
                            None
                        } else {
                            Some(pressed)
                        };
                    });
                }
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        let open = self.shown.get();
        let mut group = semantics::Node::new(self.id.clone(), cx.bounds(), semantics::Role::Group);
        for (index, section) in self.sections.iter().enumerate() {
            let mut state = semantics::State::FOCUSABLE;
            state.set(semantics::State::EXPANDED, open == Some(index));
            group = group.with_child(
                semantics::Node::new(
                    widget::Id::unique(),
                    section.header.get(),
                    semantics::Role::Button,
                )
                .with_name(section.title.text())
                .with_state(state),
            );
        }
        Some(group)
    }

    fn cursor(&self) -> Option<mouse::Interaction> {
        Some(mouse::Interaction::Pointer)
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        let open = self.shown.get();
        if let Some(section) = open.and_then(|index| self.sections.get_mut(index)) {
            visit(&mut section.body);
        }
    }
}

/// Swipeable pages, one visible at a time, bound to `page`.
pub fn carousel(pages: impl IntoIterator<Item = AnyView>, page: State<usize>) -> Carousel {
    Carousel {
        pages: pages.into_iter().map(Child::new).collect(),
        page,
        shown: page.into_prop(),
        followed: page.get(),
        position: Tween::new(page.get() as f32),
        drag: None,
        width: Cell::new(0.0),
        painted: Cell::new(page.get() as f32),
    }
}

/// See [`carousel`].
pub struct Carousel {
    pages: Vec<Child>,
    page: State<usize>,
    shown: Prop<usize>,
    followed: usize,
    position: Tween,
    /// Drag start x and the position it started from.
    drag: Option<(f32, f32, f32)>,
    width: Cell<f32>,
    painted: Cell<f32>,
}

impl View for Carousel {
    fn sizing(&self) -> Sizing {
        Sizing::FILL
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let page = self.shown.get();
        if page != self.followed {
            self.followed = page;
            self.position.go(page as f32, Instant::now());
        }

        let size = constraints.with_sizing(Sizing::FILL).max;
        let size = Size::new(
            if size.width.is_finite() {
                size.width
            } else {
                320.0
            },
            if size.height.is_finite() {
                size.height
            } else {
                160.0
            },
        );
        self.width.set(size.width);

        for (index, page) in self.pages.iter_mut().enumerate() {
            let _ = page.measure(
                cx,
                Constraints::tight(Size::new(size.width, size.height - 16.0)),
            );
            page.place(Point::new(size.width * index as f32, 0.0));
        }
        size
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let position = self
            .drag
            .map_or_else(|| self.position.value(cx.now()), |(_, _, current)| current);
        self.painted.set(position);
    }

    fn paint_over(&self, cx: &mut PaintCx<'_>) {
        let bounds = cx.bounds();
        let palette = *cx.theme().palette();
        let count = self.pages.len();
        let current = self.painted.get().round() as usize;
        let start = bounds.center().x - (count as f32 * 14.0) / 2.0;

        for index in 0..count {
            let dot = Rectangle::new(
                Point::new(start + index as f32 * 14.0, bounds.y + bounds.height - 10.0),
                Size::new(8.0, 8.0),
            );
            let color = if index == current {
                palette.primary.base.color
            } else {
                palette.background.strong.color
            };
            cx.fill(dot, color, Border::default().rounded(4.0));
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let width = self.width.get().max(1.0);
        let last = self.pages.len().saturating_sub(1) as f32;

        match look::gesture(event, cx.pointer()) {
            Some(Gesture::Down(at)) if cx.bounds().contains(at) => {
                let from = self.painted.get();
                self.drag = Some((at.x, from, from));
                Handled::No
            }
            Some(Gesture::Move(at)) => {
                let Some((start, from, _)) = self.drag else {
                    return Handled::No;
                };
                let current = (from - (at.x - start) / width).clamp(0.0, last);
                self.drag = Some((start, from, current));
                cx.request_paint();
                Handled::from((current - from).abs() > 0.01)
            }
            Some(Gesture::Up(_)) => {
                let Some((_, from, current)) = self.drag.take() else {
                    return Handled::No;
                };
                // A quarter-page swipe is enough to turn the page.
                let delta = current - from;
                let target = if delta > 0.25 {
                    from.floor() + 1.0
                } else if delta < -0.25 {
                    from.ceil() - 1.0
                } else {
                    from.round()
                }
                .clamp(0.0, last);

                self.position = Tween::new(current);
                self.position.go(target, cx.now());
                self.followed = target as usize;
                let _ = self.page.set(target as usize);
                cx.request_animation();
                Handled::from(delta.abs() > 0.01)
            }
            _ => Handled::No,
        }
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        Some(
            semantics::Node::new(widget::Id::unique(), cx.bounds(), semantics::Role::Group)
                .with_name(format!(
                    "Page {} of {}",
                    self.shown.get() + 1,
                    self.pages.len()
                )),
        )
    }

    fn clips(&self) -> bool {
        true
    }

    fn scroll_offset(&self) -> Vector {
        Vector::new(self.painted.get() * self.width.get(), 0.0)
    }

    fn is_animating(&self, now: Instant) -> bool {
        self.position.is_moving(now)
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        for page in &mut self.pages {
            visit(page);
        }
    }

    fn children_rev(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        for page in self.pages.iter_mut().rev() {
            visit(page);
        }
    }
}
