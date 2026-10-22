use crate::cx::{DescribeCx, MeasureCx, PaintCx};
use crate::layout::{Axis, Constraints, Sizing};
use crate::{AnyView, Border, Child, Color, Padding, Point, Size, View, semantics};

/// One child with optional padding, background and centering.
pub fn container(child: impl Into<AnyView>) -> Container {
    Container {
        child: Child::new(child.into()),
        padding: Padding::ZERO,
        background: None,
        border: Border::default(),
        center: false,
        sizing: Sizing::HUG,
    }
}

/// See [`container`].
pub struct Container {
    child: Child,
    padding: Padding,
    background: Option<Color>,
    border: Border,
    center: bool,
    sizing: Sizing,
}

impl Container {
    /// Space around the child.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Fills the container behind the child.
    pub fn background(mut self, color: impl Into<Color>) -> Self {
        self.background = Some(color.into());
        self
    }

    /// Outlines the container.
    pub fn border(mut self, border: Border) -> Self {
        self.border = border;
        self
    }

    /// Centers the child in the container's space.
    pub fn center(mut self) -> Self {
        self.center = true;
        self
    }

    /// Takes all available space on both axes.
    pub fn fill(mut self) -> Self {
        self.sizing = Sizing::FILL;
        self
    }

    /// Takes all available width.
    pub fn fill_width(mut self) -> Self {
        self.sizing.width = Axis::Fill(1);
        self
    }
}

impl View for Container {
    fn sizing(&self) -> Sizing {
        self.sizing
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let outer = constraints.with_sizing(self.sizing);
        let inner = outer.inset(self.padding);
        let child = self.child.measure(cx, inner.loosen());

        let content = Size::new(
            child.width.max(inner.min.width),
            child.height.max(inner.min.height),
        );
        let offset = if self.center {
            Point::new(
                (content.width - child.width) / 2.0,
                (content.height - child.height) / 2.0,
            )
        } else {
            Point::ORIGIN
        };

        self.child.place(Point::new(
            self.padding.left + offset.x,
            self.padding.top + offset.y,
        ));

        Size::new(
            content.width + self.padding.left + self.padding.right,
            content.height + self.padding.top + self.padding.bottom,
        )
    }

    fn paint(&self, cx: &mut PaintCx<'_>) {
        if self.background.is_none() && self.border.width == 0.0 {
            return;
        }

        let bounds = cx.bounds();
        let fill = self.background.unwrap_or(Color::TRANSPARENT);
        cx.fill(bounds, fill, self.border);
    }

    fn describe(&self, _cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        visit(&mut self.child);
    }
}
