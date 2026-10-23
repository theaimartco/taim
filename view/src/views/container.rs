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
        align: (0.0, 0.0),
        sizing: Sizing::HUG,
    }
}

/// See [`container`].
pub struct Container {
    child: Child,
    padding: Padding,
    background: Option<Color>,
    border: Border,
    /// Where the child sits in spare room, as fractions of it: 0 is
    /// left/top, 1 is right/bottom.
    align: (f32, f32),
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
        self.align = (0.5, 0.5);
        self
    }

    /// Centers the child horizontally.
    pub fn center_x(mut self) -> Self {
        self.align.0 = 0.5;
        self
    }

    /// Centers the child vertically.
    pub fn center_y(mut self) -> Self {
        self.align.1 = 0.5;
        self
    }

    /// Puts the child against the right edge.
    pub fn align_right(mut self) -> Self {
        self.align.0 = 1.0;
        self
    }

    /// Puts the child against the bottom edge.
    pub fn align_bottom(mut self) -> Self {
        self.align.1 = 1.0;
        self
    }

    /// Exactly this wide.
    pub fn width(mut self, width: f32) -> Self {
        self.sizing.width = Axis::Fixed(width.max(0.0));
        self
    }

    /// Exactly this tall.
    pub fn height(mut self, height: f32) -> Self {
        self.sizing.height = Axis::Fixed(height.max(0.0));
        self
    }

    /// Takes all available height.
    pub fn fill_height(mut self) -> Self {
        self.sizing.height = Axis::Fill(1);
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
        let offset = Point::new(
            (content.width - child.width) * self.align.0,
            (content.height - child.height) * self.align.1,
        );

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
