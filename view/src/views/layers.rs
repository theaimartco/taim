use crate::cx::{DescribeCx, MeasureCx, PaintCx};
use crate::layout::{Constraints, Sizing};
use crate::{AnyView, Child, Point, Size, View, semantics};

/// Children stacked on top of each other, later ones in front.
pub fn layers(children: impl IntoIterator<Item = AnyView>) -> Layers {
    Layers {
        children: children.into_iter().map(Child::new).collect(),
    }
}

/// See [`layers`].
pub struct Layers {
    children: Vec<Child>,
}

impl View for Layers {
    fn sizing(&self) -> Sizing {
        self.children.first().map_or(Sizing::HUG, Child::sizing)
    }

    fn measure(&mut self, cx: &mut MeasureCx<'_>, constraints: Constraints) -> Size {
        // The first layer sets the size; the others are fitted into it.
        let mut size = Size::ZERO;

        for (index, child) in self.children.iter_mut().enumerate() {
            let room = if index == 0 {
                constraints
            } else {
                Constraints::loose(size)
            };
            let measured = child.measure(cx, room);
            child.place(Point::ORIGIN);

            if index == 0 {
                size = measured;
            }
        }

        size
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
