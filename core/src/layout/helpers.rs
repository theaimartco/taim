//! Allocation-conscious leaf and box-model layout helpers.

use super::{Limits, Node};
use crate::{Length, Padding, Point, Size};

/// Places two nodes horizontally and centers the shorter one vertically.
pub fn next_to_each_other(
    limits: &Limits,
    spacing: f32,
    left: impl FnOnce(&Limits) -> Node,
    right: impl FnOnce(&Limits) -> Node,
) -> Node {
    let gap = finite_nonnegative(spacing);
    let left = left(limits);
    let left_size = left.size();
    let right = right(&limits.shrink(Size::new(left_size.width + gap, 0.0)));
    let right_size = right.size();
    let height = left_size.height.max(right_size.height);
    let left_y = (height - left_size.height) * 0.5;
    let right_y = (height - right_size.height) * 0.5;

    Node::with_children(
        Size::new(left_size.width + gap + right_size.width, height),
        exact_children([
            left.move_to(Point::new(0.0, left_y)),
            right.move_to(Point::new(left_size.width + gap, right_y)),
        ]),
    )
}

/// Resolves a leaf with no intrinsic extent.
pub fn atomic(limits: &Limits, width: impl Into<Length>, height: impl Into<Length>) -> Node {
    measure(limits, width.into(), height.into(), |_| Size::ZERO)
}

/// Resolves a leaf from an intrinsic-size callback.
pub fn sized(
    limits: &Limits,
    width: impl Into<Length>,
    height: impl Into<Length>,
    intrinsic: impl FnOnce(&Limits) -> Size,
) -> Node {
    measure(limits, width.into(), height.into(), intrinsic)
}

/// Wraps one measured child without over-allocating its child vector.
pub fn contained(
    limits: &Limits,
    width: impl Into<Length>,
    height: impl Into<Length>,
    layout: impl FnOnce(&Limits) -> Node,
) -> Node {
    let width = width.into();
    let height = height.into();
    let constrained = limits.width(width).height(height);
    let child = layout(&constrained);
    let resolved = constrained.resolve(width, height, child.size());

    Node::with_children(resolved, exact_children([child]))
}

/// Applies padding around one child.
pub fn padded(
    limits: &Limits,
    width: impl Into<Length>,
    height: impl Into<Length>,
    padding: impl Into<Padding>,
    layout: impl FnOnce(&Limits) -> Node,
) -> Node {
    positioned(limits, width, height, padding, layout, |node, _| node)
}

/// Applies padding and a caller-defined final child placement.
pub fn positioned(
    limits: &Limits,
    width: impl Into<Length>,
    height: impl Into<Length>,
    padding: impl Into<Padding>,
    layout: impl FnOnce(&Limits) -> Node,
    place: impl FnOnce(Node, Size) -> Node,
) -> Node {
    let width = width.into();
    let height = height.into();
    let requested_padding = padding.into();
    let outer = limits.width(width).height(height);
    let child = layout(&outer.shrink(requested_padding));
    let padding = requested_padding.fit(child.size(), outer.max());
    let inner = outer.shrink(padding);
    let inner_size = inner.resolve(width, height, child.size());
    let child = child.move_to((padding.left, padding.top));
    let child = place(child, inner_size);

    Node::with_children(inner_size.expand(padding), exact_children([child]))
}

#[inline]
fn measure(
    limits: &Limits,
    width: Length,
    height: Length,
    intrinsic: impl FnOnce(&Limits) -> Size,
) -> Node {
    let constrained = limits.width(width).height(height);
    let intrinsic = intrinsic(&constrained);
    Node::new(constrained.resolve(width, height, intrinsic))
}

#[inline]
fn exact_children<const N: usize>(children: [Node; N]) -> Vec<Node> {
    Vec::from(children)
}

#[inline]
fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_spacing_cannot_poison_a_pair_layout() {
        let limits = Limits::new(Size::ZERO, Size::new(100.0, 100.0));
        let node = next_to_each_other(
            &limits,
            f32::NAN,
            |_| Node::new(Size::new(10.0, 20.0)),
            |_| Node::new(Size::new(30.0, 10.0)),
        );

        assert_eq!(node.size(), Size::new(40.0, 20.0));
        assert_eq!(node.children()[1].bounds().x, 10.0);
    }
}
