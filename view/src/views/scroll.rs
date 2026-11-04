use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::{Axis, Constraints, Sizing};
use crate::{AnyView, Border, Child, Event, Handled, Point, Rectangle, Size, Vector, View};
use crate::{mouse, semantics};

use taim_core::widget;

/// Shows `content` in a vertically scrollable area that fills its parent.
pub fn scroll(content: impl Into<AnyView>) -> Scroll {
    Scroll {
        content: Child::new(content.into()),
        offset: 0.0,
        viewport: Size::ZERO,
        horizontal: false,
        id: widget::Id::unique(),
    }
}

/// See [`scroll`].
pub struct Scroll {
    content: Child,
    offset: f32,
    viewport: Size,
    horizontal: bool,
    id: widget::Id,
}

/// Pixels per wheel "line".
const LINE: f32 = 40.0;
const BAR: f32 = 6.0;

impl Scroll {
    /// Scrolls left/right, filling width and hugging content height.
    pub fn horizontal(mut self) -> Self {
        self.horizontal = true;
        self
    }

    fn extent(&self, size: Size) -> f32 {
        if self.horizontal {
            size.width
        } else {
            size.height
        }
    }

    fn max_offset(&self) -> f32 {
        (self.extent(self.content.size()) - self.extent(self.viewport)).max(0.0)
    }

    fn scroll_by(&mut self, delta: f32) -> bool {
        if !delta.is_finite() {
            return false;
        }
        let next = (self.offset - delta).clamp(0.0, self.max_offset());
        let moved = next != self.offset;
        self.offset = next;
        moved
    }
}

impl View for Scroll {
    fn sizing(&self) -> Sizing {
        if self.horizontal {
            Sizing {
                width: Axis::Fill(1),
                height: Axis::Hug,
            }
        } else {
            Sizing::FILL
        }
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let outer = constraints.with_sizing(self.sizing());
        let room = if self.horizontal {
            Size::new(f32::INFINITY, outer.max.height)
        } else {
            Size::new(outer.max.width, f32::INFINITY)
        };
        let content = self.content.measure(cx, Constraints::loose(room));

        self.viewport = outer.clamp(Size::new(
            if outer.max.width.is_finite() {
                outer.max.width
            } else {
                content.width
            },
            if !self.horizontal && outer.max.height.is_finite() {
                outer.max.height
            } else {
                content.height
            },
        ));
        self.content.place(Point::ORIGIN);
        self.offset = self.offset.min(self.max_offset());
        self.viewport
    }

    fn paint(&self, _cx: &mut PaintCx<'_>) {}

    fn paint_over(&self, cx: &mut PaintCx<'_>) {
        let total = self.extent(self.content.size());
        let span = self.extent(self.viewport);
        if total <= span || span <= 0.0 {
            return;
        }

        let bounds = cx.bounds();
        let length = (span * (span / total)).max(24.0).min(span);
        let shift = (span - length) * (self.offset / self.max_offset());
        let bar = if self.horizontal {
            Rectangle::new(
                Point::new(
                    bounds.x + shift,
                    bounds.y + (bounds.height - BAR - 2.0).max(0.0),
                ),
                Size::new(length, BAR.min(bounds.height)),
            )
        } else {
            Rectangle::new(
                Point::new(
                    bounds.x + (bounds.width - BAR - 2.0).max(0.0),
                    bounds.y + shift,
                ),
                Size::new(BAR.min(bounds.width), length),
            )
        };

        let color = cx
            .theme()
            .palette()
            .background
            .strongest
            .color
            .scale_alpha(0.5);
        cx.fill(bar, color, Border::default().rounded(BAR / 2.0));
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event else {
            return Handled::No;
        };
        if !cx.is_hovered() {
            return Handled::No;
        }

        let component = |x: f32, y: f32| if self.horizontal && x != 0.0 { x } else { y };
        let pixels = match *delta {
            mouse::ScrollDelta::Lines { x, y } => component(x, y) * LINE,
            mouse::ScrollDelta::Pixels { x, y } => component(x, y),
        };

        let moved = self.scroll_by(pixels);
        if moved {
            cx.request_paint();
        }
        // At an edge let a scrollable ancestor consume the wheel.
        Handled::from(moved)
    }

    fn describe(&self, cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        Some(semantics::Node::new(
            self.id.clone(),
            cx.bounds(),
            semantics::Role::ScrollView,
        ))
    }

    fn clips(&self) -> bool {
        true
    }

    fn scroll_offset(&self) -> Vector {
        if self.horizontal {
            Vector::new(self.offset, 0.0)
        } else {
            Vector::new(0.0, self.offset)
        }
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        visit(&mut self.content);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::space;

    #[test]
    fn each_axis_clamps_independently_and_rejects_non_finite_wheel_input() {
        for horizontal in [false, true] {
            let mut view = scroll(space());
            view.horizontal = horizontal;
            *view.content.frame_mut() = Rectangle::with_size(Size::new(300.0, 200.0));
            view.viewport = Size::new(100.0, 50.0);
            let maximum = if horizontal { 200.0 } else { 150.0 };
            assert!(!view.scroll_by(f32::NAN));
            assert!(!view.scroll_by(f32::INFINITY));
            assert!(view.scroll_by(-1000.0));
            assert_eq!(view.offset, maximum);
            assert!(
                !view.scroll_by(-10.0),
                "an edge leaves the event for its parent"
            );
            assert_eq!(
                view.scroll_offset(),
                if horizontal {
                    Vector::new(maximum, 0.0)
                } else {
                    Vector::new(0.0, maximum)
                }
            );
            assert!(view.scroll_by(1000.0));
            assert_eq!(view.offset, 0.0);
        }
    }
}
