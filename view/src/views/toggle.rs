use std::cell::Cell;

use super::look::{self, POP, PRESS, SNAP, Spring};
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
    pressed: Option<look::Press>,
    /// How far on it is drawn, `0` off and `1` on; springs between them.
    /// A cell so paint can follow a value changed elsewhere.
    progress: Cell<Spring>,
    /// `1` while held: the switch's knob stretches, as on iOS.
    hold: Spring,
    id: widget::Id,
}

impl Toggle {
    fn new(shape: Shape, label: impl IntoProp<String>, value: State<bool>) -> Self {
        let name = label.into_prop();
        let on = f32::from(u8::from(value.get()));
        let tuning = match shape {
            Shape::Checkbox => POP,
            Shape::Switch => SNAP,
        };

        Self {
            shape,
            label: Child::new(Text::from_prop(name.clone())),
            name,
            value,
            shown: value.into_prop(),
            enabled: Prop::fixed(true),
            gap: 8.0,
            tick: look::Label::glyph(super::Icon::CHECK, 14.0),
            pressed: None,
            progress: Cell::new(Spring::new(on, tuning)),
            hold: Spring::new(0.0, PRESS),
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
        cx.request_animation();
    }
}

impl View for Toggle {
    fn cancel_interaction(&mut self) {
        self.pressed = None;
        self.hold.set(0.0);
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        if !self.enabled.get() {
            self.pressed = None;
        }
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
        use taim_core::Renderer as _;

        let on = self.shown.get();
        let enabled = self.enabled.get();
        let palette = *cx.theme().palette();
        let area = self.indicator(cx.bounds());
        let now = cx.now();

        // Follow the value wherever it was changed from.
        let mut progress = self.progress.get();
        let target = f32::from(u8::from(on));
        if (progress.target() - target).abs() > f32::EPSILON {
            progress.go(target, now);
            self.progress.set(progress);
        }
        let travel = progress.value(now);
        let amount = travel.clamp(0.0, 1.0);

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
                let fill = palette.background.base.color.mix(accent, amount);
                let edge = idle_edge.mix(accent, amount);
                cx.fill(
                    area,
                    fill,
                    Border::default().width(1.5).color(edge).rounded(4.0),
                );

                // The tick pops in, overshooting a little, and shrinks away.
                if travel > 0.02 {
                    let ink = if enabled {
                        palette.primary.base.text
                    } else {
                        palette.secondary.base.text
                    };
                    let centre = area.center();
                    let scale = travel.max(0.0);
                    cx.renderer().start_transformation(
                        taim_core::Transformation::translate(centre.x, centre.y)
                            * taim_core::Transformation::scale(scale)
                            * taim_core::Transformation::translate(-centre.x, -centre.y),
                    );
                    self.tick.paint_centered(cx, area, ink);
                    cx.renderer().end_transformation();
                }
            }
            Shape::Switch => {
                let track = palette.background.strong.color.mix(accent, amount);
                let radius = area.height / 2.0;
                cx.fill(area, track, Border::default().rounded(radius));

                // The knob stretches while held and slides with a snap.
                let knob = area.height - 4.0;
                let width = knob + 7.0 * self.hold.value(now).max(0.0);
                let left = area.x + 2.0;
                let right = area.x + area.width - width - 2.0;
                let x = left + (right - left) * travel;
                cx.fill_shadowed(
                    Rectangle::new(Point::new(x, area.y + 2.0), Size::new(width, knob)),
                    Color::WHITE,
                    Border::default().rounded(knob / 2.0),
                    crate::Shadow {
                        color: Color::from_rgba(0.0, 0.0, 0.0, 0.18),
                        offset: crate::Vector::new(0.0, 1.0),
                        blur_radius: 3.0,
                    },
                );
            }
        }

        if cx.has_focus() {
            look::focus_ring(cx, area, 4.0);
        }
    }

    fn is_animating(&self, now: crate::Instant) -> bool {
        self.progress.get().is_moving(now) || self.hold.is_moving(now)
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        if self.pressed.is_some_and(|press| press.cancelled_by(event)) {
            self.pressed = None;
            self.hold.go(0.0, cx.now());
            cx.request_animation();
            return Handled::No;
        }
        if !self.enabled.get() {
            self.pressed = None;
            return Handled::No;
        }
        if let Some(press) = look::Press::start(event)
            && cx.is_hovered()
            && self.pressed.is_none()
        {
            self.pressed = Some(press);
            self.hold.go(1.0, cx.now());
            cx.request_focus();
            cx.request_animation();
            return Handled::Yes;
        }
        if self.pressed.is_some_and(|press| press.released_by(event)) {
            self.pressed = None;
            self.hold.go(0.0, cx.now());
            cx.request_animation();
            if cx.is_hovered() {
                self.flip(cx);
            }
            return Handled::Yes;
        }
        if cx.has_focus() && look::is_activation(event) {
            self.flip(cx);
            return Handled::Yes;
        }
        if matches!(
            event,
            Event::Mouse(mouse::Event::CursorMoved { .. })
                | Event::Touch(taim_core::touch::Event::FingerMoved { .. })
        ) {
            cx.request_paint();
        }
        Handled::No
    }

    fn accessibility_action(
        &mut self,
        cx: &mut EventCx<'_>,
        request: &semantics::Request,
    ) -> Handled {
        match request {
            semantics::Request::Activate if self.enabled.get() => {
                self.pressed = None;
                cx.request_focus();
                self.flip(cx);
                Handled::Yes
            }
            semantics::Request::Focus if self.focusable() => {
                cx.request_focus();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        let mut state = look::interactive_state(self.enabled.get(), cx.has_focus());
        state.set(semantics::State::CHECKED, self.shown.get());

        Some(
            semantics::Node::new(self.id.clone(), cx.bounds(), semantics::Role::Checkbox)
                .with_name(self.name.get())
                .with_state(state)
                .with_actions(if self.enabled.get() {
                    semantics::Actions::ACTIVATE | semantics::Actions::FOCUS
                } else {
                    semantics::Actions::empty()
                }),
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
