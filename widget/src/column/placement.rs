//! TAIM-native, linear-time placement for wrapping vertical flows.
//!
//! Measurement and placement have different responsibilities: children are
//! measured by their widgets first, then placed into explicit column groups.
//! The planner never infers group boundaries from already-positioned nodes,
//! preventing padding and vertical alignment from corrupting wrap grouping.

use crate::core::alignment::{self, Alignment};
use crate::core::layout::Node;
use crate::core::{Padding, Point, Size};

/// A measured vertical group and its range in the original child order.
#[derive(Debug, Clone, Copy)]
struct Group {
    start: usize,
    end: usize,
    width: f32,
    height: f32,
    x: f32,
}

fn finite_extent(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

// Defend against f32 overflow when an untrusted custom widget reports a
// near-MAX intrinsic extent. Returned layout coordinates stay finite.
fn finite_sum(a: f32, b: f32) -> f32 {
    let result = a + b;
    if result.is_finite() {
        result
    } else {
        f32::MAX
    }
}

fn alignment_fraction(align: Alignment) -> f32 {
    match align {
        Alignment::Start => 0.0,
        Alignment::Center => 0.5,
        Alignment::End => 1.0,
    }
}

fn vertical_fraction(align: alignment::Vertical) -> f32 {
    match align {
        alignment::Vertical::Top => 0.0,
        alignment::Vertical::Center => 0.5,
        alignment::Vertical::Bottom => 1.0,
    }
}

/// Arrange measured child nodes in columns, preserving their original order.
///
/// The column width is the widest measured child in that group; the column
/// height includes spacing only between children. Oversize nodes begin their
/// own group instead of generating an empty first column. Only a finite
/// positive max height constrains wrapping; positive infinity is unbounded.
pub(super) fn arrange(
    mut nodes: Vec<Node>,
    max_height: f32,
    vertical_spacing: f32,
    horizontal_spacing: f32,
    padding: Padding,
    horizontal_alignment: Alignment,
    vertical_alignment: alignment::Vertical,
) -> (Size, Vec<Node>) {
    if nodes.is_empty() {
        return (Size::ZERO, nodes);
    }

    let gap = finite_extent(vertical_spacing);
    let gutter = finite_extent(horizontal_spacing);
    let padding = padding.normalized();
    let vertical_limit = if max_height.is_nan() {
        0.0
    } else {
        max_height.max(0.0)
    };

    let mut groups: Vec<Group> = Vec::new();

    // 1. Build explicit contiguous groups; no node is measured again.
    for (index, node) in nodes.iter().enumerate() {
        let size = node.size();
        let width = finite_extent(size.width);
        let height = finite_extent(size.height);

        if let Some(last) = groups.last_mut() {
            let proposed = finite_sum(last.height, finite_sum(gap, height));
            if proposed <= vertical_limit || vertical_limit.is_infinite() {
                last.height = proposed;
                last.width = last.width.max(width);
                last.end = index + 1;
                continue;
            }
        }

        groups.push(Group {
            start: index,
            end: index + 1,
            width,
            height,
            x: 0.0,
        });
    }

    // 2. Assign X offsets. Never add a trailing gutter to intrinsic width.
    let mut x = 0.0;
    let mut total_height = 0.0_f32;
    for (index, group) in groups.iter_mut().enumerate() {
        if index != 0 {
            x = finite_sum(x, gutter);
        }
        group.x = x;
        x = finite_sum(x, group.width);
        total_height = total_height.max(group.height);
    }

    // Never subtract the terminal gutter from a saturated sum:
    // MAX + MAX -> MAX, but MAX - MAX incorrectly produces zero width.
    let total_width = x;
    let align_x = alignment_fraction(horizontal_alignment);
    let align_y = vertical_fraction(vertical_alignment);

    // 3. Align within each group using its explicit end index. This handles
    // nonzero padding, singleton groups, and unequal group heights correctly.
    for group in groups {
        let y_shift = (total_height - group.height).max(0.0) * align_y;
        let mut y = 0.0;
        for (offset, node) in nodes[group.start..group.end].iter_mut().enumerate() {
            if offset > 0 {
                y = finite_sum(y, gap);
            }
            let width = finite_extent(node.size().width);
            let x_shift = (group.width - width).max(0.0) * align_x;
            node.move_to_mut(Point::new(
                finite_sum(padding.left, finite_sum(group.x, x_shift)),
                finite_sum(padding.top, finite_sum(y_shift, y)),
            ));
            y = finite_sum(y, finite_extent(node.size().height));
        }
    }

    (Size::new(total_width, total_height), nodes)
}


/// Place measured horizontal-flow children into explicit row groups.
///
/// Rows have independent heights; the tallest determines the intrinsic
/// vertical span of its group, and only inter-row gaps contribute to the
/// intrinsic total. Padding does not enter row-break detection.
pub(crate) fn arrange_rows(
    mut nodes: Vec<Node>,
    max_width: f32,
    horizontal_spacing: f32,
    vertical_spacing: f32,
    padding: Padding,
    vertical_alignment: Alignment,
    horizontal_alignment: alignment::Horizontal,
) -> (Size, Vec<Node>) {
    if nodes.is_empty() {
        return (Size::ZERO, nodes);
    }

    let gap = finite_extent(horizontal_spacing);
    let gutter = finite_extent(vertical_spacing);
    let padding = padding.normalized();
    let horizontal_limit = if max_width.is_nan() {
        0.0
    } else {
        max_width.max(0.0)
    };

    #[derive(Debug)]
    struct RowGroup {
        start: usize,
        end: usize,
        width: f32,
        height: f32,
        y: f32,
    }

    let mut groups: Vec<RowGroup> = Vec::new();
    for (index, node) in nodes.iter().enumerate() {
        let size = node.size();
        let width = finite_extent(size.width);
        let height = finite_extent(size.height);

        if let Some(last) = groups.last_mut() {
            let next_width = finite_sum(last.width, finite_sum(gap, width));
            if next_width <= horizontal_limit || horizontal_limit.is_infinite() {
                last.width = next_width;
                last.height = last.height.max(height);
                last.end = index + 1;
                continue;
            }
        }

        groups.push(RowGroup {
            start: index,
            end: index + 1,
            width,
            height,
            y: 0.0,
        });
    }

    let mut y = 0.0;
    let mut widest = 0.0_f32;
    for (index, row) in groups.iter_mut().enumerate() {
        if index != 0 {
            y = finite_sum(y, gutter);
        }
        row.y = y;
        widest = widest.max(row.width);
        y = finite_sum(y, row.height);
    }

    let full_height = y;
    let cross_fraction = alignment_fraction(vertical_alignment);
    let row_fraction = match horizontal_alignment {
        alignment::Horizontal::Left => 0.0,
        alignment::Horizontal::Center => 0.5,
        alignment::Horizontal::Right => 1.0,
    };

    for row in groups {
        let x_offset = (widest - row.width).max(0.0) * row_fraction;
        let mut x = 0.0;
        for (offset, node) in nodes[row.start..row.end].iter_mut().enumerate() {
            if offset > 0 {
                x = finite_sum(x, gap);
            }

            let node_height = finite_extent(node.size().height);
            let y_offset = (row.height - node_height).max(0.0) * cross_fraction;

            node.move_to_mut(Point::new(
                finite_sum(padding.left, finite_sum(x_offset, x)),
                finite_sum(padding.top, finite_sum(row.y, y_offset)),
            ));

            x = finite_sum(x, finite_extent(node.size().width));
        }
    }

    (Size::new(widest, full_height), nodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(width: f32, height: f32) -> Node {
        Node::new(Size::new(width, height))
    }



    #[test]
    fn extreme_intergroup_spacing_never_cancels_intrinsic_extent() {
        let (size, placed) = arrange(
            vec![node(10.0, 5.0), node(20.0, 5.0)],
            6.0,
            1.0,
            f32::MAX,
            Padding::ZERO,
            Alignment::Start,
            alignment::Vertical::Top,
        );
        assert_eq!(size.width, f32::MAX);
        assert!(placed.iter().all(|node| node.bounds().x.is_finite()));

        let (size, placed) = arrange_rows(
            vec![node(5.0, 10.0), node(5.0, 20.0)],
            6.0,
            1.0,
            f32::MAX,
            Padding::ZERO,
            Alignment::Start,
            alignment::Horizontal::Left,
        );
        assert_eq!(size.height, f32::MAX);
        assert!(placed.iter().all(|node| node.bounds().y.is_finite()));
    }

    #[test]
    fn extreme_custom_widget_sizes_never_produce_infinite_positions() {
        let (column_size, column) = arrange(
            vec![node(f32::MAX, 3.0), node(f32::MAX, 3.0)],
            4.0,
            2.0,
            5.0,
            Padding::ZERO,
            Alignment::Start,
            alignment::Vertical::Top,
        );
        assert!(column_size.width.is_finite());
        assert!(column.iter().all(|child| {
            let position = child.bounds().position();
            position.x.is_finite() && position.y.is_finite()
        }));

        let (row_size, row) = arrange_rows(
            vec![node(3.0, f32::MAX), node(3.0, f32::MAX)],
            4.0,
            2.0,
            5.0,
            Padding::ZERO,
            Alignment::Start,
            alignment::Horizontal::Left,
        );
        assert!(row_size.height.is_finite());
        assert!(row.iter().all(|child| {
            let position = child.bounds().position();
            position.x.is_finite() && position.y.is_finite()
        }));
    }

    #[test]
    fn row_wrapping_has_explicit_breaks_and_correct_dimensions() {
        let (size, placed) = arrange_rows(
            vec![node(20.0, 10.0), node(20.0, 20.0), node(30.0, 5.0)],
            45.0,
            5.0,
            7.0,
            Padding::ZERO,
            Alignment::Start,
            alignment::Horizontal::Left,
        );
        assert_eq!(size, Size::new(45.0, 32.0));
        assert_eq!(placed[0].bounds().position(), Point::ORIGIN);
        assert_eq!(placed[1].bounds().position(), Point::new(25.0, 0.0));
        assert_eq!(placed[2].bounds().position(), Point::new(0.0, 27.0));
    }

    #[test]
    fn row_horizontal_and_vertical_alignment_respect_padding() {
        let (size, placed) = arrange_rows(
            vec![node(20.0, 10.0), node(20.0, 20.0), node(30.0, 5.0)],
            45.0,
            5.0,
            7.0,
            Padding::sides(3.0, 0.0, 0.0, 2.0),
            Alignment::Center,
            alignment::Horizontal::Center,
        );
        assert_eq!(size, Size::new(45.0, 32.0));
        assert_eq!(placed[0].bounds().position(), Point::new(2.0, 8.0));
        assert_eq!(placed[1].bounds().position(), Point::new(27.0, 3.0));
        assert_eq!(placed[2].bounds().position(), Point::new(9.5, 30.0));
    }

    #[test]
    fn row_oversized_item_does_not_leave_empty_front_group() {
        let (size, placed) = arrange_rows(
            vec![node(100.0, 10.0), node(4.0, 8.0)],
            45.0,
            5.0,
            3.0,
            Padding::ZERO,
            Alignment::End,
            alignment::Horizontal::Right,
        );
        assert_eq!(size, Size::new(100.0, 21.0));
        assert_eq!(placed[0].bounds().position(), Point::ORIGIN);
        assert_eq!(placed[1].bounds().position(), Point::new(96.0, 13.0));
    }

    #[test]
    fn preserves_order_and_assigns_exact_group_boundaries() {
        let (size, placed) = arrange(
            vec![node(10.0, 20.0), node(20.0, 20.0), node(5.0, 30.0)],
            45.0,
            5.0,
            7.0,
            Padding::ZERO,
            Alignment::Start,
            alignment::Vertical::Top,
        );
        assert_eq!(size, Size::new(32.0, 45.0));
        assert_eq!(placed[0].bounds().position(), Point::new(0.0, 0.0));
        assert_eq!(placed[1].bounds().position(), Point::new(0.0, 25.0));
        assert_eq!(placed[2].bounds().position(), Point::new(27.0, 0.0));
    }

    #[test]
    fn aligns_each_group_correctly_with_nonzero_padding() {
        let (size, placed) = arrange(
            vec![node(10.0, 20.0), node(20.0, 20.0), node(5.0, 30.0)],
            45.0,
            5.0,
            7.0,
            Padding::sides(3.0, 0.0, 0.0, 2.0),
            Alignment::Center,
            alignment::Vertical::Center,
        );
        assert_eq!(size, Size::new(32.0, 45.0));
        assert_eq!(placed[0].bounds().position(), Point::new(7.0, 3.0));
        assert_eq!(placed[1].bounds().position(), Point::new(2.0, 28.0));
        assert_eq!(placed[2].bounds().position(), Point::new(29.0, 10.5));
    }

    #[test]
    fn one_oversized_child_does_not_create_an_empty_first_group() {
        let (size, placed) = arrange(
            vec![node(10.0, 100.0), node(8.0, 4.0)],
            25.0,
            5.0,
            3.0,
            Padding::ZERO,
            Alignment::End,
            alignment::Vertical::Bottom,
        );
        assert_eq!(size, Size::new(21.0, 100.0));
        assert_eq!(placed[0].bounds().position(), Point::ORIGIN);
        assert_eq!(placed[1].bounds().position(), Point::new(13.0, 96.0));
    }

    #[test]
    fn empty_and_unbounded_flows_require_no_sentinel_nodes() {
        let (empty, children) = arrange(
            Vec::new(), f32::INFINITY, 3.0, 4.0, Padding::ZERO,
            Alignment::Start, alignment::Vertical::Top,
        );
        assert_eq!(empty, Size::ZERO);
        assert!(children.is_empty());

        let (size, placed) = arrange(
            vec![node(2.0, 3.0), node(5.0, 7.0)],
            f32::INFINITY,
            4.0,
            8.0,
            Padding::ZERO,
            Alignment::Start,
            alignment::Vertical::Top,
        );
        assert_eq!(size, Size::new(5.0, 14.0));
        assert_eq!(placed[1].bounds().position(), Point::new(0.0, 7.0));
    }

    #[test]
    fn malformed_spacing_does_not_enter_node_coordinates() {
        let (size, placed) = arrange(
            vec![node(2.0, 3.0), node(4.0, 5.0)],
            30.0,
            f32::NAN,
            f32::INFINITY,
            Padding::ZERO,
            Alignment::Start,
            alignment::Vertical::Top,
        );
        assert_eq!(size, Size::new(4.0, 8.0));
        assert_eq!(placed[1].bounds().position(), Point::new(0.0, 3.0));
    }
}
