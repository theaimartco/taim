use super::look;
use super::text::Text;
use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::Constraints;
use crate::state::{IntoProp, Prop, State};
use crate::{Border, Child, Color, Event, Handled, Point, Rectangle, Size, View};
use crate::{mouse, semantics};

use taim_core::widget;

/// A box that is ticked when `checked` is true; clicking it flips `checked`.
pub fn checkbox(label: impl IntoProp<String>, checked: State<bool>) -> Toggle {
    Toggle::new(Shape::Checkbox, label, checked)
}

/// An on/off switch bound to `on`; clicking it flips `on`.
pub fn switch(label: impl IntoProp<String>, on: State<bool>) -> Toggle {
    Toggle::new(Shape::Switch, label, on)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Checkbox,
    Switch,
}

impl Shape {
    fn indicator(self) -> Size {
        match self {
            Self::Checkbox => Size::new(18.0, 18.0),
            Self::Switch => Size::new(36.0, 20.0),
        }
    }
}

/// See [`checkbox`] and [`switch`].
pub struct Toggle {
    shape: Shape,
    label: Child,
    name: Prop<String>,
    value: State<bool>,
    shown: Prop<bool>,
    enabled: Prop<bool>,
    gap: f32,
    /// The checkbox's tick, from the icon font.
    tick: look::Label,
    pressed: bool,
    id: widget::Id,
}

impl Toggle {
    fn new(shape: Shape, label: impl IntoProp<String>, value: State<bool>) -> Self {
        let name = label.into_prop();

        Self {
            shape,
            label: Child::new(Text::from_prop(name.clone())),
            name,
            value,
            shown: value.into_prop(),
            enabled: Prop::fixed(true),
            gap: 8.0,
            tick: look::Label::glyph(super::Icon::CHECK, 14.0),
            pressed: false,
            id: widget::Id::unique(),
        }
    }

    /// Whether the control can be changed; a closure follows state.
    pub fn enabled(mut self, enabled: impl IntoProp<bool>) -> Self {
        self.enabled = enabled.into_prop();
        self
    }

    fn indicator(&self, bounds: Rectangle) -> Rectangle {
        let size = self.shape.indicator();
        Rectangle::new(
            Point::new(bounds.x, bounds.y + (bounds.height - size.height) / 2.0),
            size,
        )
    }

    fn flip(&self, cx: &mut EventCx<'_>) {
        self.value.update(|value| *value = !*value);
        cx.request_paint();
    }
}

impl View for Toggle {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let indicator = self.shape.indicator();
        let offset = indicator.width + self.gap;
        let room = Size::new(
            (constraints.max.width - offset).max(0.0),
            constraints.max.height,
        );
        let label = self.label.measure(cx, Constraints::loose(room));
        if self.shape == Shape::Checkbox {
            let _ = self.tick.measure(cx);
        }

        let height = label.height.max(indicator.height);
        self.label
            .place(Point::new(offset, (height - label.height) / 2.0));

        Size::new(offset + label.width, height)
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let on = self.shown.get();
        let enabled = self.enabled.get();
        let palette = *cx.theme().palette();
        let area = self.indicator(cx.bounds());

        let accent = if enabled {
            palette.primary.base.color
        } else {
            palette.secondary.base.color
        };
        let hovered = cx.is_hovered() && enabled;
        let idle_edge = if hovered {
            palette.background.strongest.color
        } else {
            palette.background.strong.color
        };

        match self.shape {
            Shape::Checkbox => {
                let (fill, edge) = if on {
                    (accent, accent)
                } else {
                    (palette.background.base.color, idle_edge)
                };
                cx.fill(
                    area,
                    fill,
                    Border::default().width(1.5).color(edge).rounded(4.0),
                );

                if on {
                    let ink = if enabled {
                        palette.primary.base.text
                    } else {
                        palette.secondary.base.text
                    };
                    self.tick.paint_centered(cx, area, ink);
                }
            }
            Shape::Switch => {
                let track = if on {
                    accent
                } else {
                    palette.background.strong.color
                };
                let radius = area.height / 2.0;
                cx.fill(area, track, Border::default().rounded(radius));

                let knob = area.height - 4.0;
                let x = if on {
                    area.x + area.width - knob - 2.0
                } else {
                    area.x + 2.0
                };
                cx.fill(
                    Rectangle::new(Point::new(x, area.y + 2.0), Size::new(knob, knob)),
                    Color::WHITE,
                    Border::default().rounded(knob / 2.0),
                );
            }
        }

        if cx.has_focus() {
            look::focus_ring(cx, area, 4.0);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        if !self.enabled.get() {
            return Handled::No;
        }

        if look::is_press(event) && cx.is_hovered() {
            self.pressed = true;
            cx.request_focus();
            return Handled::Yes;
        }

        if look::is_release(event) && self.pressed {
            self.pressed = false;
            if cx.is_hovered() {
                self.flip(cx);
            }
            return Handled::Yes;
        }

        if cx.has_focus() && look::is_activation(event) {
            self.flip(cx);
            return Handled::Yes;
        }

        if matches!(event, Event::Mouse(mouse::Event::CursorMoved { .. })) {
            cx.request_paint();
        }

        Handled::No
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        let mut state = look::interactive_state(self.enabled.get(), cx.has_focus());
        state.set(semantics::State::CHECKED, self.shown.get());

        Some(
            semantics::Node::new(self.id.clone(), cx.bounds(), semantics::Role::Checkbox)
                .with_name(self.name.get())
                .with_state(state)
                .with_actions(semantics::Actions::ACTIVATE),
        )
    }

    fn cursor(&self) -> Option<mouse::Interaction> {
        self.enabled.get().then_some(mouse::Interaction::Pointer)
    }

    fn focusable(&self) -> bool {
        self.enabled.get()
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        visit(&mut self.label);
    }
}
