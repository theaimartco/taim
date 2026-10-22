use crate::cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
use crate::layout::{Constraints, Sizing};
use crate::{AnyView, Border, Child, Event, Handled, Point, Rectangle, Size, Vector, View};
use crate::{mouse, semantics};

use taim_core::widget;

/// Shows `content` in a vertically scrollable area that fills its parent.
pub fn scroll(content: impl Into<AnyView>) -> Scroll {
    Scroll {
        content: Child::new(content.into()),
        offset: 0.0,
        viewport: Size::ZERO,
        id: widget::Id::unique(),
    }
}

/// See [`scroll`].
pub struct Scroll {
    content: Child,
    offset: f32,
    viewport: Size,
    id: widget::Id,
}

/// Pixels per wheel "line".
const LINE: f32 = 40.0;
const BAR: f32 = 6.0;

impl Scroll {
    fn max_offset(&self) -> f32 {
        (self.content.size().height - self.viewport.height).max(0.0)
    }

    fn scroll_by(&mut self, delta: f32) -> bool {
        let next = (self.offset - delta).clamp(0.0, self.max_offset());
        let moved = next != self.offset;
        self.offset = next;
        moved
    }
}

impl View for Scroll {
    fn sizing(&self) -> Sizing {
        Sizing::FILL
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let outer = constraints.with_sizing(Sizing::FILL);
        let room = Size::new(outer.max.width, f32::INFINITY);
        let content = self.content.measure(cx, Constraints::loose(room));

        self.viewport = Size::new(
            if outer.max.width.is_finite() {
                outer.max.width
            } else {
                content.width
            },
            if outer.max.height.is_finite() {
                outer.max.height
            } else {
                content.height
            },
        );
        self.content.place(Point::ORIGIN);
        self.offset = self.offset.min(self.max_offset());
        self.viewport
    }

    fn paint(&self, _cx: &mut PaintCx<'_>) {}

    fn paint_over(&self, cx: &mut PaintCx<'_>) {
        let total = self.content.size().height;
        if total <= self.viewport.height || total <= 0.0 {
            return;
        }

        let bounds = cx.bounds();
        let length = (bounds.height * bounds.height / total).max(24.0);
        let travel = bounds.height - length;
        let top = bounds.y + travel * (self.offset / self.max_offset());
        let bar = Rectangle::new(
            Point::new(bounds.x + bounds.width - BAR - 2.0, top),
            Size::new(BAR, length),
        );

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

        let pixels = match *delta {
            mouse::ScrollDelta::Lines { y, .. } => y * LINE,
            mouse::ScrollDelta::Pixels { y, .. } => y,
        };

        if self.scroll_by(pixels) {
            cx.request_paint();
        }
        Handled::Yes
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
        Vector::new(0.0, self.offset)
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        visit(&mut self.content);
    }
}
