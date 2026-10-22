use crate::cx::{DescribeCx, MeasureCx, PaintCx};
use crate::layout::{Axis, Constraints, Sizing};
use crate::{AnyView, Child, Padding, Point, Size, View, semantics};

/// Children laid out top to bottom.
pub fn column(children: impl IntoIterator<Item = AnyView>) -> Stack {
    Stack::new(Direction::Down, children)
}

/// Children laid out left to right.
pub fn row(children: impl IntoIterator<Item = AnyView>) -> Stack {
    Stack::new(Direction::Right, children)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Down,
    Right,
}

/// Where children sit across the stacking direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    /// Left of a column, top of a row.
    #[default]
    Start,
    /// Centered.
    Center,
    /// Right of a column, bottom of a row.
    End,
}

/// A [`column`] or [`row`].
pub struct Stack {
    direction: Direction,
    children: Vec<Child>,
    gap: f32,
    padding: Padding,
    align: Align,
    sizing: Sizing,
}

impl Stack {
    fn new(direction: Direction, children: impl IntoIterator<Item = AnyView>) -> Self {
        Self {
            direction,
            children: children.into_iter().map(Child::new).collect(),
            gap: 0.0,
            padding: Padding::ZERO,
            align: Align::Start,
            sizing: Sizing::HUG,
        }
    }

    /// Adds a child at the end.
    pub fn push(mut self, child: impl Into<AnyView>) -> Self {
        self.children.push(Child::new(child.into()));
        self
    }

    /// Space between consecutive children.
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.max(0.0);
        self
    }

    /// Space around the children.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Cross-axis placement of children.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Fill the available width.
    pub fn fill_width(mut self) -> Self {
        self.sizing.width = Axis::Fill(1);
        self
    }

    /// Fill the available height.
    pub fn fill_height(mut self) -> Self {
        self.sizing.height = Axis::Fill(1);
        self
    }

    /// Splits a size into (along the stack, across it).
    fn split(&self, size: Size) -> (f32, f32) {
        match self.direction {
            Direction::Down => (size.height, size.width),
            Direction::Right => (size.width, size.height),
        }
    }

    /// Joins (along, across) back into a size.
    fn join(&self, along: f32, across: f32) -> Size {
        match self.direction {
            Direction::Down => Size::new(across, along),
            Direction::Right => Size::new(along, across),
        }
    }

    fn fill_weight(&self, child: &Child) -> Option<u16> {
        let sizing = child.sizing();
        match self.direction {
            Direction::Down => sizing.height.fill_weight(),
            Direction::Right => sizing.width.fill_weight(),
        }
    }
}

impl View for Stack {
    fn sizing(&self) -> Sizing {
        self.sizing
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        let inner = constraints.with_sizing(self.sizing).inset(self.padding);
        let (max_along, max_across) = self.split(inner.max);
        let gaps = self.gap * self.children.len().saturating_sub(1) as f32;

        // First pass: everything that does not fill, with unbounded length.
        let mut used = gaps;
        let mut across: f32 = 0.0;
        let mut total_weight = 0_u32;

        for index in 0..self.children.len() {
            if let Some(weight) = self.fill_weight(&self.children[index]) {
                total_weight += u32::from(weight);
                continue;
            }

            let room = self.join(f32::INFINITY, max_across);
            let size = self.children[index].measure(cx, Constraints::loose(room));
            let (length, thickness) = self.split(size);
            used += length;
            across = across.max(thickness);
        }

        // Second pass: fill children share what is left, in proportion.
        let left = if max_along.is_finite() {
            (max_along - used).max(0.0)
        } else {
            0.0
        };

        for index in 0..self.children.len() {
            let Some(weight) = self.fill_weight(&self.children[index]) else {
                continue;
            };

            let share = left * f32::from(weight) / total_weight as f32;
            let room = self.join(share, max_across);
            let size = self.children[index].measure(cx, Constraints::loose(room));
            let (length, thickness) = self.split(size);
            used += length;
            across = across.max(thickness);
        }

        // Placement.
        let (min_along, min_across) = self.split(inner.min);
        let across = across.max(min_across);
        let mut cursor = 0.0;

        for index in 0..self.children.len() {
            let (length, thickness) = self.split(self.children[index].size());
            let offset = match self.align {
                Align::Start => 0.0,
                Align::Center => (across - thickness) / 2.0,
                Align::End => across - thickness,
            };

            let local = self.join(cursor, offset);
            self.children[index].place(Point::new(
                self.padding.left + local.width,
                self.padding.top + local.height,
            ));
            cursor += length + self.gap;
        }

        let content = self.join(used.max(min_along), across);
        Size::new(
            content.width + self.padding.left + self.padding.right,
            content.height + self.padding.top + self.padding.bottom,
        )
    }

    fn paint(&self, _cx: &mut PaintCx<'_>) {}

    fn describe(&self, _cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }

    fn children(&mut self, visit: &mut dyn FnMut(&mut Child)) {
        for child in &mut self.children {
            visit(child);
        }
    }
}
