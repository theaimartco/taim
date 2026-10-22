use super::look;
use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::{Axis, Constraints, Sizing};
use crate::state::{IntoProp, Prop, State};
use crate::{Border, Color, Event, Handled, Point, Rectangle, Size, View};
use crate::{keyboard, mouse, semantics};

use std::ops::RangeInclusive;
use taim_core::widget;

/// A horizontal slider that moves `value` within `range`.
pub fn slider(range: RangeInclusive<f32>, value: State<f32>) -> Slider {
    Slider {
        range,
        value,
        shown: value.into_prop(),
        step: None,
        dragging: false,
        hovered: false,
        id: widget::Id::unique(),
    }
}

/// See [`slider`].
pub struct Slider {
    range: RangeInclusive<f32>,
    value: State<f32>,
    shown: Prop<f32>,
    step: Option<f32>,
    dragging: bool,
    hovered: bool,
    id: widget::Id,
}

const TRACK: f32 = 4.0;
const THUMB: f32 = 16.0;

impl Slider {
    /// Snaps the value to multiples of `step` from the start of the range;
    /// also the arrow-key increment.
    pub fn step(mut self, step: f32) -> Self {
        self.step = (step > 0.0).then_some(step);
        self
    }

    fn span(&self) -> f32 {
        (self.range.end() - self.range.start()).max(f32::EPSILON)
    }

    fn snap(&self, raw: f32) -> f32 {
        let start = *self.range.start();
        let snapped = match self.step {
            Some(step) => start + ((raw - start) / step).round() * step,
            None => raw,
        };
        snapped.clamp(start, *self.range.end())
    }

    fn set_from_pointer(&self, bounds: Rectangle, x: f32) {
        let usable = (bounds.width - THUMB).max(1.0);
        let fraction = ((x - bounds.x - THUMB / 2.0) / usable).clamp(0.0, 1.0);
        let _ = self
            .value
            .set(self.snap(self.range.start() + fraction * self.span()));
    }
}

impl View for Slider {
    fn sizing(&self) -> Sizing {
        Sizing {
            width: Axis::Fill(1),
            height: Axis::Fixed(THUMB),
        }
    }

    fn measure(&mut self, _cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let sized = constraints.with_sizing(self.sizing());
        let width = if sized.max.width.is_finite() {
            sized.max.width
        } else {
            200.0
        };
        Size::new(width, THUMB)
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let bounds = cx.bounds();
        let palette = *cx.theme().palette();
        let fraction = ((self.shown.get() - self.range.start()) / self.span()).clamp(0.0, 1.0);
        let usable = (bounds.width - THUMB).max(0.0);
        let center_y = bounds.y + bounds.height / 2.0;

        let track = Rectangle::new(
            Point::new(bounds.x + THUMB / 2.0, center_y - TRACK / 2.0),
            Size::new(usable, TRACK),
        );
        let filled = Rectangle::new(track.position(), Size::new(usable * fraction, TRACK));
        let thumb = Rectangle::new(
            Point::new(bounds.x + usable * fraction, center_y - THUMB / 2.0),
            Size::new(THUMB, THUMB),
        );

        let rounded = Border::default().rounded(TRACK / 2.0);
        cx.fill(track, palette.background.strong.color, rounded);
        cx.fill(filled, palette.primary.base.color, rounded);

        let edge = if self.hovered || self.dragging {
            palette.primary.strong.color
        } else {
            palette.primary.base.color
        };
        cx.fill(
            thumb,
            Color::WHITE,
            Border::default()
                .width(2.0)
                .color(edge)
                .rounded(THUMB / 2.0),
        );

        if cx.has_focus() {
            look::focus_ring(cx, thumb, THUMB / 2.0);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let bounds = cx.bounds();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if cx.is_hovered() => {
                self.dragging = true;
                cx.request_focus();
                if let Some(at) = cx.pointer() {
                    self.set_from_pointer(bounds, at.x);
                }
                Handled::Yes
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if self.dragging {
                    self.set_from_pointer(bounds, position.x);
                    return Handled::Yes;
                }

                let hovered = cx.is_hovered();
                if hovered != self.hovered {
                    self.hovered = hovered;
                    cx.request_paint();
                }
                Handled::No
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if self.dragging => {
                self.dragging = false;
                cx.request_paint();
                Handled::Yes
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(named),
                ..
            }) if cx.has_focus() => {
                use keyboard::key::Named;

                let step = self.step.unwrap_or(self.span() / 100.0);
                let current = self.value.get();
                let next = match named {
                    Named::ArrowLeft | Named::ArrowDown => current - step,
                    Named::ArrowRight | Named::ArrowUp => current + step,
                    Named::Home => *self.range.start(),
                    Named::End => *self.range.end(),
                    _ => return Handled::No,
                };
                let _ = self.value.set(self.snap(next));
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
        Some(if self.dragging {
            mouse::Interaction::Grabbing
        } else {
            mouse::Interaction::Grab
        })
    }

    fn focusable(&self) -> bool {
        true
    }
}

/// A bar showing how far `value` is through `range`.
pub fn progress(range: RangeInclusive<f32>, value: impl IntoProp<f32>) -> Progress {
    Progress {
        range,
        value: value.into_prop(),
        id: widget::Id::unique(),
    }
}

/// See [`progress`].
pub struct Progress {
    range: RangeInclusive<f32>,
    value: Prop<f32>,
    id: widget::Id,
}

impl Progress {
    fn fraction(&self) -> f32 {
        let span = (self.range.end() - self.range.start()).max(f32::EPSILON);
        ((self.value.get() - self.range.start()) / span).clamp(0.0, 1.0)
    }
}

impl View for Progress {
    fn sizing(&self) -> Sizing {
        Sizing {
            width: Axis::Fill(1),
            height: Axis::Fixed(8.0),
        }
    }

    fn measure(&mut self, _cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let sized = constraints.with_sizing(self.sizing());
        let width = if sized.max.width.is_finite() {
            sized.max.width
        } else {
            200.0
        };
        Size::new(width, sized.min.height)
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let bounds = cx.bounds();
        let palette = *cx.theme().palette();
        let rounded = Border::default().rounded(bounds.height / 2.0);
        let done = Rectangle::new(
            bounds.position(),
            Size::new(bounds.width * self.fraction(), bounds.height),
        );

        cx.fill(bounds, palette.background.strong.color, rounded);
        cx.fill(done, palette.primary.base.color, rounded);
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        Some(
            semantics::Node::new(self.id.clone(), cx.bounds(), semantics::Role::Slider)
                .with_state(semantics::State::BUSY)
                .with_value(semantics::Value::Number {
                    current: f64::from(self.value.get()),
                    min: f64::from(*self.range.start()),
                    max: f64::from(*self.range.end()),
                }),
        )
    }
}

/// A thin separating line across the available width.
pub fn rule() -> Rule {
    Rule { thickness: 1.0 }
}

/// See [`rule`].
#[derive(Debug, Clone, Copy)]
pub struct Rule {
    thickness: f32,
}

impl Rule {
    /// Line thickness in logical pixels.
    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness.max(0.0);
        self
    }
}

impl View for Rule {
    fn sizing(&self) -> Sizing {
        Sizing {
            width: Axis::Fill(1),
            height: Axis::Fixed(self.thickness),
        }
    }

    fn measure(&mut self, _cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let sized = constraints.with_sizing(self.sizing());
        Size::new(sized.max.width.min(1.0e6), self.thickness)
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        let bounds = cx.bounds();
        let color = cx.theme().palette().background.strong.color;
        cx.fill(bounds, color, Border::default());
    }

    fn describe(&self, _cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Scope;
    use std::sync::Arc;

    #[test]
    fn slider_snaps_and_clamps() {
        let mut scope = Scope::new(Arc::default());
        let value = scope.signal(0.0_f32);
        let slider = slider(0.0..=10.0, value).step(2.5);

        assert_eq!(slider.snap(3.6), 2.5);
        assert_eq!(slider.snap(4.0), 5.0);
        assert_eq!(slider.snap(-3.0), 0.0);
        assert_eq!(slider.snap(99.0), 10.0);

        let bounds = Rectangle::new(Point::ORIGIN, Size::new(116.0, THUMB));
        slider.set_from_pointer(bounds, 58.0);
        assert_eq!(value.get(), 5.0);
    }

    #[test]
    fn progress_fraction_is_bounded() {
        assert_eq!(progress(0.0..=4.0, 1.0).fraction(), 0.25);
        assert_eq!(progress(0.0..=4.0, 9.0).fraction(), 1.0);
        assert_eq!(progress(5.0..=5.0, 5.0).fraction(), 0.0);
    }
}
