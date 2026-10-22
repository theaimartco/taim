use crate::cx::{DescribeCx, MeasureCx, PaintCx};
use crate::layout::{Axis, Constraints, Sizing};
use crate::{Size, View, semantics};

/// Empty space. Fills whatever room its parent leaves unless given a size.
pub fn space() -> Space {
    Space {
        sizing: Sizing::FILL,
    }
}

/// See [`space`].
#[derive(Debug, Clone, Copy)]
pub struct Space {
    sizing: Sizing,
}

impl Space {
    /// A fixed width.
    pub fn width(mut self, width: f32) -> Self {
        self.sizing.width = Axis::Fixed(width);
        self
    }

    /// A fixed height.
    pub fn height(mut self, height: f32) -> Self {
        self.sizing.height = Axis::Fixed(height);
        self
    }
}

impl View for Space {
    fn sizing(&self) -> Sizing {
        self.sizing
    }

    fn measure(&mut self, _cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        constraints.with_sizing(self.sizing).min
    }

    fn paint(&self, _cx: &mut PaintCx<'_>) {}

    fn describe(&self, _cx: &DescribeCx<'_>) -> Option<semantics::Node> {
        None
    }
}
