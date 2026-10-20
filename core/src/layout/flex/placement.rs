//! TAIM's axis-independent linear placement of measured layout nodes.
//!
//! Measurement and placement have separate responsibilities: the planner
//! assigns sizing requests; this module turns resulting extents into bounded
//! positions without calling widget layout again. Prefix positions use f64,
//! preventing overflowing f32 addition from poisoning later siblings.

use super::{Alignment, Axis, Length, Limits, Node, Padding, Size};

/// Inputs needed for the final, one-pass positioning stage.
pub(super) struct Frame<'a> {
    pub(super) axis: Axis,
    pub(super) limits: &'a Limits,
    pub(super) width: Length,
    pub(super) height: Length,
    pub(super) padding: Padding,
    pub(super) spacing: f32,
    pub(super) alignment: Alignment,
    pub(super) cross_extent: f32,
}

impl Frame<'_> {
    /// Place already-measured nodes, then resolve the group envelope.
    ///
    /// No child layout or allocation of another node vector happens here.
    pub(super) fn finish(self, mut children: Vec<Node>) -> Node {
        let (pad_main, pad_cross) = self.axis.pack(self.padding.left, self.padding.top);
        let cross = match self.axis {
            Axis::Horizontal => self.limits.resolve_height(self.height, self.cross_extent),
            Axis::Vertical => self.limits.resolve_width(self.width, self.cross_extent),
        };

        let gap = extent(self.spacing);
        let mut prefix = 0.0_f64;
        for (index, child) in children.iter_mut().enumerate() {
            if index != 0 {
                prefix += f64::from(gap);
            }
            let offset = coordinate(f64::from(extent(pad_main)) + prefix);
            child.move_to_mut(self.axis.pack(offset, pad_cross));

            // Align *within* the resolved cross span, not within the raw
            // measured maximum, so Fill/Fixed cross policies remain sound.
            match self.axis {
                Axis::Horizontal => {
                    child.align_mut(Alignment::Start, self.alignment, Size::new(0.0, cross));
                }
                Axis::Vertical => {
                    child.align_mut(self.alignment, Alignment::Start, Size::new(cross, 0.0));
                }
            }

            prefix += f64::from(extent(self.axis.main(child.size())));
        }

        let intrinsic = coordinate(prefix);
        let main = match self.axis {
            Axis::Horizontal => self.limits.resolve_width(self.width, intrinsic),
            Axis::Vertical => self.limits.resolve_height(self.height, intrinsic),
        };

        let size = Size::from(self.axis.pack(main, cross));
        let baseline = first_baseline(&self.axis, &children);
        let result = Node::with_children(size.expand(self.padding), children);
        match baseline {
            Some(value) => result.with_baseline(coordinate(f64::from(value) + f64::from(extent(self.padding.top)))),
            None => result,
        }
    }
}

/// Nonnegative finite values are the only legal child extents and gutters.
#[inline]
fn extent(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

/// Layout coordinate saturation is explicit; f32 casts cannot yield infinity.
#[inline]
fn coordinate(value: f64) -> f32 {
    value.clamp(0.0, f64::from(f32::MAX)) as f32
}

fn first_baseline(axis: &Axis, children: &[Node]) -> Option<f32> {
    let baseline_at = |node: &Node| {
        let baseline = node.baseline()?;
        let y = node.bounds().y;
        if !baseline.is_finite() || !y.is_finite() {
            return None;
        }
        Some(coordinate(f64::from(y) + f64::from(baseline)))
    };
    match axis {
        Axis::Horizontal => children.iter().filter_map(baseline_at).reduce(f32::max),
        Axis::Vertical => children.first().and_then(baseline_at),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Point;

    fn arrange(axis: Axis, alignment: Alignment, gap: f32, cross: f32, children: Vec<Node>) -> Node {
        Frame {
            axis,
            limits: &Limits::new(Size::ZERO, Size::INFINITE),
            width: Length::Fit,
            height: Length::Fit,
            padding: Padding::ZERO,
            spacing: gap,
            alignment,
            cross_extent: cross,
        }
        .finish(children)
    }

    #[test]
    fn row_uses_prefix_extents_and_cross_alignment() {
        let result = arrange(
            Axis::Horizontal,
            Alignment::Center,
            5.0,
            20.0,
            vec![Node::new(Size::new(20.0, 8.0)), Node::new(Size::new(30.0, 20.0))],
        );
        assert_eq!(result.size(), Size::new(55.0, 20.0));
        assert_eq!(result.children()[0].bounds().position(), Point::new(0.0, 6.0));
        assert_eq!(result.children()[1].bounds().position(), Point::new(25.0, 0.0));
    }

    #[test]
    fn column_uses_vertical_prefix_and_horizontal_alignment() {
        let result = arrange(
            Axis::Vertical,
            Alignment::End,
            3.0,
            40.0,
            vec![Node::new(Size::new(10.0, 12.0)), Node::new(Size::new(40.0, 8.0))],
        );
        assert_eq!(result.size(), Size::new(40.0, 23.0));
        assert_eq!(result.children()[0].bounds().position(), Point::new(30.0, 0.0));
        assert_eq!(result.children()[1].bounds().position(), Point::new(0.0, 15.0));
    }

    #[test]
    fn huge_siblings_saturate_without_infinite_positions() {
        let result = arrange(
            Axis::Horizontal,
            Alignment::Start,
            f32::MAX,
            1.0,
            vec![Node::new(Size::new(f32::MAX, 1.0)), Node::new(Size::new(f32::MAX, 1.0))],
        );
        assert_eq!(result.size().width, f32::MAX);
        assert!(result.children().iter().all(|c| c.bounds().x.is_finite()));
        assert_eq!(result.children()[1].bounds().x, f32::MAX);
    }

    #[test]
    fn malformed_spacing_and_child_extent_cannot_poison_envelope() {
        let result = arrange(
            Axis::Vertical,
            Alignment::Start,
            f32::NAN,
            8.0,
            vec![Node::new(Size::new(8.0, f32::NAN)), Node::new(Size::new(8.0, 5.0))],
        );
        assert_eq!(result.size(), Size::new(8.0, 5.0));
        assert_eq!(result.children()[1].bounds().y, 0.0);
    }

    #[test]
    fn row_baseline_tracks_cross_position_and_column_uses_first_child() {
        let row = arrange(
            Axis::Horizontal,
            Alignment::Center,
            0.0,
            30.0,
            vec![
                Node::new(Size::new(10.0, 20.0)).with_baseline(10.0),
                Node::new(Size::new(10.0, 30.0)).with_baseline(18.0),
            ],
        );
        assert_eq!(row.baseline(), Some(18.0));
        let column = arrange(
            Axis::Vertical,
            Alignment::Start,
            0.0,
            10.0,
            vec![
                Node::new(Size::new(10.0, 5.0)),
                Node::new(Size::new(10.0, 5.0)).with_baseline(3.0),
            ],
        );
        assert_eq!(column.baseline(), None);
    }

    #[test]
    fn empty_group_has_zero_intrinsic_extent() {
        let row = arrange(Axis::Horizontal, Alignment::Start, 100.0, 0.0, vec![]);
        assert_eq!(row.size(), Size::ZERO);
        assert!(row.children().is_empty());
        assert_eq!(row.baseline(), None);
    }
}
