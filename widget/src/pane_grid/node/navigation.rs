use super::Node;

use crate::core::{Point, Rectangle, Size};
use crate::pane_grid::{Direction, Pane};

const NAVIGATION_EXTENT: f32 = 4096.0;

impl Node {
    /// Finds the pane immediately across one edge without materializing the
    /// complete region map. The two tree walks are allocation-free and keep
    /// navigation linear even for deeply split workspaces.
    pub(crate) fn adjacent_pane(&self, pane: Pane, direction: Direction) -> Option<Pane> {
        let bounds = Rectangle::new(
            Point::ORIGIN,
            Size::new(NAVIGATION_EXTENT, NAVIGATION_EXTENT),
        );
        let current = self.region_of(pane, bounds)?;
        let target = direction.probe(current);
        let mut adjacent = None;

        self.pane_at(target, bounds, &mut adjacent);
        adjacent
    }

    fn region_of(&self, target: Pane, bounds: Rectangle) -> Option<Rectangle> {
        match self {
            Self::Pane(pane) => (*pane == target).then_some(bounds),
            Self::Split {
                axis, ratio, a, b, ..
            } => {
                let (a_bounds, b_bounds, _) = axis.split(&bounds, *ratio, 0.0, 0.0, 0.0);
                a.region_of(target, a_bounds)
                    .or_else(|| b.region_of(target, b_bounds))
            }
        }
    }

    fn pane_at(&self, target: Point, bounds: Rectangle, found: &mut Option<Pane>) {
        match self {
            Self::Pane(pane) if bounds.contains(target) => {
                if found.is_none_or(|current| *pane < current) {
                    *found = Some(*pane);
                }
            }
            Self::Split {
                axis, ratio, a, b, ..
            } => {
                let (a_bounds, b_bounds, _) = axis.split(&bounds, *ratio, 0.0, 0.0, 0.0);
                a.pane_at(target, a_bounds, found);
                b.pane_at(target, b_bounds, found);
            }
            Self::Pane(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane_grid::{Axis, Split};

    #[test]
    fn centerline_probe_targets_correct_subdivided_neighbor() {
        // The right-hand side is split 25%/75%; the left pane's
        // center points into the lower-right pane. Sampling near the
        // top-left corner would incorrectly choose Pane(1).
        let workspace = Node::Split {
            id: Split(4),
            axis: Axis::Vertical,
            ratio: 0.5,
            a: Box::new(Node::Pane(Pane(0))),
            b: Box::new(Node::Split {
                id: Split(3),
                axis: Axis::Horizontal,
                ratio: 0.25,
                a: Box::new(Node::Pane(Pane(1))),
                b: Box::new(Node::Pane(Pane(2))),
            }),
        };

        assert_eq!(
            workspace.adjacent_pane(Pane(0), Direction::Right),
            Some(Pane(2))
        );
        assert_eq!(
            workspace.adjacent_pane(Pane(2), Direction::Left),
            Some(Pane(0))
        );
    }

    #[test]
    fn navigation_does_not_wrap_at_workspace_boundary() {
        let workspace = Node::Pane(Pane(0));

        assert_eq!(
            workspace.adjacent_pane(Pane(0), Direction::Right),
            None
        );
        assert_eq!(
            workspace.adjacent_pane(Pane(0), Direction::Up),
            None
        );
    }
}
