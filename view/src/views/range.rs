use super::look;
use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::{Axis, Constraints, Sizing};
use crate::state::{IntoProp, Prop, State};
use crate::{Border, Color, Event, Handled, Point, Rectangle, Size, View};
use crate::{keyboard, mouse, semantics};

use std::ops::RangeInclusive;
use taim_core::widget;

// Public ranges must never introduce NaN coordinates or invalid clamp bounds.
fn normalized_range(range: RangeInclusive<f32>) -> RangeInclusive<f32> {
    let start = if range.start().is_finite() {
        *range.start()
    } else {
        0.0
    };
    let end = if range.end().is_finite() {
        *range.end()
    } else {
        start
    };
    start.min(end)..=start.max(end)
}

fn range_fraction(range: &RangeInclusive<f32>, value: f32) -> f32 {
    let span = f64::from(*range.end()) - f64::from(*range.start());
    if value.is_nan() || span <= 0.0 {
        return 0.0;
    }
    ((f64::from(value) - f64::from(*range.start())) / span).clamp(0.0, 1.0) as f32
}

/// A horizontal slider that moves `value` within `range`.
/// Reversed endpoints are ordered; non-finite endpoints collapse to a finite range.
pub fn slider(range: RangeInclusive<f32>, value: State<f32>) -> Slider {
    Slider {
        range: normalized_range(range),
        value,
        shown: value.into_prop(),
        step: None,
        dragging: None,
        hovered: false,
        grip: look::Spring::new(0.0, look::POP),
        id: widget::Id::unique(),
    }
}

/// See [`slider`].
pub struct Slider {
    range: RangeInclusive<f32>,
    value: State<f32>,
    shown: Prop<f32>,
    step: Option<f32>,
    dragging: Option<look::Press>,
    hovered: bool,
    /// 0 at rest, 1 while held: the thumb swells under the finger.
    grip: look::Spring,
    id: widget::Id,
}

const TRACK: f32 = 4.0;
const THUMB: f32 = 16.0;

impl Slider {
    /// Snaps the value to multiples of `step` from the start of the range;
    /// also the arrow-key increment.
    pub fn step(mut self, step: f32) -> Self {
        self.step = (step.is_finite() && step > 0.0).then_some(step);
        self
    }

    fn span(&self) -> f64 {
        (f64::from(*self.range.end()) - f64::from(*self.range.start())).max(f64::from(f32::EPSILON))
    }

    fn snap(&self, raw: f32) -> f32 {
        let start = f64::from(*self.range.start());
        let end = f64::from(*self.range.end());
        let raw = if raw.is_nan() {
            start
        } else {
            f64::from(raw).clamp(start, end)
        };
        let snapped = match self.step {
            Some(step) => start + ((raw - start) / f64::from(step)).round() * f64::from(step),
            None => raw,
        };
        snapped.clamp(start, end) as f32
    }

    fn set_from_pointer(&self, bounds: Rectangle, x: f32) {
        if !x.is_finite() || !bounds.x.is_finite() || !bounds.width.is_finite() {
            return;
        }
        let usable = (bounds.width - THUMB).max(1.0);
        let fraction = ((x - bounds.x - THUMB / 2.0) / usable).clamp(0.0, 1.0);
        let _ = self.value.set(self.snap(
            (f64::from(*self.range.start())
                + f64::from(fraction)
                    * (f64::from(*self.range.end()) - f64::from(*self.range.start())))
                as f32,
        ));
    }
}

impl View for Slider {
    fn cancel_interaction(&mut self) {
        self.dragging = None;
        self.hovered = false;
        self.grip.set(0.0);
    }

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
        let fraction = range_fraction(&self.range, self.shown.get());
        let usable = (bounds.width - THUMB).max(0.0);
        let center_y = bounds.y + bounds.height / 2.0;

        let track = Rectangle::new(
            Point::new(bounds.x + THUMB / 2.0, center_y - TRACK / 2.0),
            Size::new(usable, TRACK),
        );
        let filled = Rectangle::new(track.position(), Size::new(usable * fraction, TRACK));
        // Held, the thumb swells by up to a third around its centre.
        let grown = THUMB * (1.0 + 0.35 * self.grip.value(cx.now()).max(-0.2));
        let thumb_x = bounds.x + usable * fraction + THUMB / 2.0;
        let thumb = Rectangle::new(
            Point::new(thumb_x - grown / 2.0, center_y - grown / 2.0),
            Size::new(grown, grown),
        );

        let rounded = Border::default().rounded(TRACK / 2.0);
        cx.fill(track, palette.background.strong.color, rounded);
        cx.fill(filled, palette.primary.base.color, rounded);

        let edge = if self.hovered || self.dragging.is_some() {
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
                .rounded(grown / 2.0),
        );

        if cx.has_focus() {
            look::focus_ring(cx, thumb, grown / 2.0);
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        if self.dragging.is_some_and(|press| press.cancelled_by(event)) {
            self.dragging = None;
            self.hovered = false;
            self.grip.go(0.0, cx.now());
            cx.request_paint();
            return Handled::No;
        }
        let bounds = cx.bounds();
        if let Some(press) = look::Press::start(event)
            && cx.is_hovered()
            && self.dragging.is_none()
        {
            self.dragging = Some(press);
            self.grip.go(1.0, cx.now());
            cx.request_focus();
            if let Some(at) = cx.pointer() {
                self.set_from_pointer(bounds, at.x);
            }
            cx.request_paint();
            return Handled::Yes;
        }
        if let Some(press) = self.dragging {
            if let Some(position) = press.movement(event) {
                self.set_from_pointer(bounds, position.x);
                return Handled::Yes;
            }
            if press.released_by(event) {
                self.dragging = None;
                self.grip.go(0.0, cx.now());
                cx.request_paint();
                return Handled::Yes;
            }
        }
        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let hovered = cx.is_hovered();
                if hovered != self.hovered {
                    self.hovered = hovered;
                    cx.request_paint();
                }
                Handled::No
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(named),
                ..
            }) if cx.has_focus() => {
                use keyboard::key::Named;

                let step = self.step.unwrap_or((self.span() / 100.0) as f32);
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
        Some(if self.dragging.is_some() {
            mouse::Interaction::Grabbing
        } else {
            mouse::Interaction::Grab
        })
    }

    fn focusable(&self) -> bool {
        true
    }

    fn is_animating(&self, now: crate::Instant) -> bool {
        self.grip.is_moving(now)
    }
}

/// A bar showing how far `value` is through `range`.
/// Reversed endpoints are ordered and non-finite endpoints are normalized.
pub fn progress(range: RangeInclusive<f32>, value: impl IntoProp<f32>) -> Progress {
    Progress {
        range: normalized_range(range),
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
        range_fraction(&self.range, self.value.get())
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
    fn malformed_and_extreme_ranges_remain_finite() {
        let mut scope = Scope::new(Arc::default());
        let value = scope.signal(0.0_f32);
        for range in [10.0..=0.0, f32::NAN..=f32::INFINITY, -f32::MAX..=f32::MAX] {
            let slider = slider(range.clone(), value).step(f32::INFINITY);
            assert!(slider.snap(f32::NAN).is_finite());
            assert!(slider.snap(f32::MAX).is_finite());
            slider.set_from_pointer(Rectangle::with_size(Size::new(116.0, THUMB)), 58.0);
            assert!(value.get().is_finite());
            assert!(progress(range, f32::NAN).fraction().is_finite());
        }
        assert_eq!(progress(-f32::MAX..=f32::MAX, 0.0).fraction(), 0.5);
        let slider = slider(-f32::MAX..=f32::MAX, value);
        slider.set_from_pointer(Rectangle::with_size(Size::new(116.0, THUMB)), 58.0);
        assert_eq!(value.get(), 0.0);
        slider.set_from_pointer(Rectangle::with_size(Size::new(116.0, THUMB)), f32::NAN);
        assert_eq!(value.get(), 0.0);
    }

    #[test]
    fn progress_fraction_is_bounded() {
        assert_eq!(progress(0.0..=4.0, 1.0).fraction(), 0.25);
        assert_eq!(progress(0.0..=4.0, 9.0).fraction(), 1.0);
        assert_eq!(progress(5.0..=5.0, 5.0).fraction(), 0.0);
    }
}
