//! Two fingers spreading or pinching over a view.
use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::{Constraints, Sizing};
use crate::{AnyView, Child, Event, Handled, Point, Size, View, mouse, semantics};

use taim_core::touch;

type OnPinch = Box<dyn Fn(f32, Point)>;
type OnEnd = Box<dyn Fn(f32)>;

/// Wraps `child` and reports two-finger pinches over it: the scale since
/// the fingers landed (above 1 spreading, below 1 pinching) and the point
/// between them, relative to the view.
///
/// The second finger landing makes it a pinch: from then on the gesture
/// belongs to this view, and a press or scroll the first finger began in
/// the child is cancelled.
///
/// ```ignore
/// let zoom = cx.signal(1.0_f32);
/// let base = cx.signal(1.0_f32);
/// pinch(image(handle))
///     .on_pinch(move |scale, _| { let _ = zoom.set(base.get() * scale); })
///     .on_end(move |_| { let _ = base.set(zoom.get()); })
/// ```
pub fn pinch(child: impl Into<AnyView>) -> Pinch {
    Pinch {
        child: Child::new(child.into()),
        fingers: Vec::new(),
        start: None,
        on_pinch: None,
        on_end: None,
        scale: 1.0,
    }
}

/// See [`pinch`].
pub struct Pinch {
    child: Child,
    /// Fingers down on the view, in the order they landed.
    fingers: Vec<(touch::Finger, Point)>,
    /// The distance between the first two fingers when both were down.
    start: Option<f32>,
    on_pinch: Option<OnPinch>,
    on_end: Option<OnEnd>,
    /// The latest scale, reported again when the pinch ends.
    scale: f32,
}

impl Pinch {
    /// Runs `action` with the scale and the focal point (relative to the
    /// view) each time the fingers move.
    pub fn on_pinch(mut self, action: impl Fn(f32, Point) + 'static) -> Self {
        self.on_pinch = Some(Box::new(action));
        self
    }

    /// Runs `action` with the final scale when a finger lifts.
    pub fn on_end(mut self, action: impl Fn(f32) + 'static) -> Self {
        self.on_end = Some(Box::new(action));
        self
    }

    fn pinching(&self) -> bool {
        self.start.is_some()
    }

    fn pair(&self) -> Option<(Point, Point)> {
        Some((self.fingers.first()?.1, self.fingers.get(1)?.1))
    }

    fn end(&mut self) {
        if self.start.take().is_some()
            && let Some(end) = &self.on_end
        {
            end(self.scale);
        }
        self.scale = 1.0;
    }
}

impl View for Pinch {
    fn sizing(&self) -> Sizing {
        self.child.view().sizing()
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let size = self.child.measure(cx, constraints);
        self.child.place(Point::ORIGIN);
        size
    }

    fn paint(&self, _cx: &mut PaintCx<'_>) {}

    fn intercept(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::Touch(touch::Event::FingerPressed { id, position, .. })
                if cx.bounds().contains(*position) =>
            {
                self.fingers.retain(|(finger, _)| finger != id);
                self.fingers.push((*id, *position));

                if !self.pinching()
                    && let Some((a, b)) = self.pair()
                {
                    self.start = Some(a.distance(b).max(1.0));
                    self.scale = 1.0;
                    cx.request_paint();
                    return Handled::Yes;
                }
                Handled::from(self.pinching())
            }
            Event::Touch(touch::Event::FingerMoved { id, position, .. }) => {
                let Some(finger) = self.fingers.iter_mut().find(|(finger, _)| finger == id) else {
                    return Handled::No;
                };
                finger.1 = *position;

                let (Some(start), Some((a, b))) = (self.start, self.pair()) else {
                    return Handled::No;
                };
                self.scale = a.distance(b) / start;

                if let Some(action) = &self.on_pinch {
                    let bounds = cx.bounds();
                    let focal =
                        Point::new((a.x + b.x) / 2.0 - bounds.x, (a.y + b.y) / 2.0 - bounds.y);
                    action(self.scale, focal);
                }
                cx.request_paint();
                Handled::Yes
            }
            Event::Touch(
                touch::Event::FingerLifted { id, .. } | touch::Event::FingerLost { id, .. },
            ) => {
                let was = self.pinching();
                self.fingers.retain(|(finger, _)| finger != id);
                if self.fingers.len() < 2 {
                    self.end();
                }
                Handled::from(was)
            }
            // While pinching, the first finger's pointer events belong to
            // the pinch too: the child must not scroll or press under it.
            Event::Mouse(
                mouse::Event::CursorMoved { .. }
                | mouse::Event::ButtonPressed(_)
                | mouse::Event::ButtonReleased(_),
            ) if self.pinching() && cx.is_touch() => Handled::Yes,
            _ => Handled::No,
        }
    }

    fn describe(&self, _cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        visit(&mut self.child);
    }
}
