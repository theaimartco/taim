use crate::{Alignment, Padding, Point, Rectangle, Size, Transformation, Vector};

/// Paint-only layout metadata kept away from the traversal-hot portion of a
/// node. Most nodes are neither transformed nor clipped, so paying for both
/// option payloads in every flex tree wastes cache lines.
#[derive(Debug, Clone, Default, PartialEq)]
struct Effects {
    transform: Option<Transformation>,
    clip: Option<Rectangle>,
}

/// The bounds of an element and its children.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Node {
    bounds: Rectangle,
    children: Vec<Node>,
    baseline: Option<f32>,
    effects: Option<Box<Effects>>,
}

impl Node {
    /// Creates a new [`Node`] with the given [`Size`].
    pub const fn new(size: Size) -> Self {
        Self::with_children(size, Vec::new())
    }

    /// Creates a new [`Node`] with the given [`Size`] and children.
    pub const fn with_children(size: Size, children: Vec<Node>) -> Self {
        Node {
            bounds: Rectangle {
                x: 0.0,
                y: 0.0,
                width: size.width,
                height: size.height,
            },
            children,
            baseline: None,
            effects: None,
        }
    }

    /// Creates a new [`Node`] that wraps a single child with some [`Padding`].
    pub fn container(child: Self, padding: Padding) -> Self {
        let size = child.bounds.size().expand(padding);
        let children = vec![child.move_to(Point::new(padding.left, padding.top))];
        Self::with_children(size, children)
    }

    /// Returns the [`Size`] of the [`Node`].
    pub fn size(&self) -> Size {
        Size::new(self.bounds.width, self.bounds.height)
    }

    /// Returns the bounds of the [`Node`].
    pub fn bounds(&self) -> Rectangle {
        self.bounds
    }

    /// Returns the children of the [`Node`].
    pub fn children(&self) -> &[Node] {
        &self.children
    }

    /// Returns the first-baseline offset of this [`Node`] from its own
    /// top edge, if one was set with [`Node::with_baseline`].
    ///
    /// `None` means the node has no meaningful baseline of its own (e.g.
    /// most non-text content); a container aligning children by baseline
    /// should fall back to the node's bottom edge (`size().height`) in
    /// that case, matching the behavior of an ordinary bottom/center/top
    /// alignment for content that opts out of baseline alignment.
    pub fn baseline(&self) -> Option<f32> {
        self.baseline
    }

    /// Sets the first-baseline offset of this [`Node`], measured from its
    /// own top edge (before any [`Node::move_to`]/[`Node::translate`]
    /// repositioning -- the baseline moves rigidly with the node).
    pub fn with_baseline(mut self, baseline: f32) -> Self {
        self.baseline = Some(baseline);
        self
    }

    /// Returns the [`Transformation`] applied to this [`Node`] in
    /// addition to its [`Node::bounds`] position, if any.
    ///
    /// This lets a container (e.g. `stack`) record a child's rotation,
    /// scale, or other transform alongside its bounds, so a cache
    /// invalidated only by bounds changes does not also need to be
    /// invalidated by transform-only changes.
    pub fn transform(&self) -> Option<Transformation> {
        self.effects.as_deref().and_then(|effects| effects.transform)
    }

    /// Sets the [`Transformation`] applied to this [`Node`], in addition
    /// to its bounds.
    pub fn with_transform(mut self, transform: Transformation) -> Self {
        self.effects_mut().transform = Some(transform);
        self
    }

    /// Returns the clip [`Rectangle`] that should be applied while
    /// painting this [`Node`] and its children, if any -- in the same
    /// coordinate space as [`Node::bounds`].
    pub fn clip(&self) -> Option<Rectangle> {
        self.effects.as_deref().and_then(|effects| effects.clip)
    }

    /// Sets the clip [`Rectangle`] for this [`Node`].
    pub fn with_clip(mut self, clip: Rectangle) -> Self {
        self.effects_mut().clip = Some(clip);
        self
    }

    /// Aligns the [`Node`] in the given space.
    pub fn align(mut self, align_x: Alignment, align_y: Alignment, space: Size) -> Self {
        self.align_mut(align_x, align_y, space);
        self
    }

    /// Mutable reference version of [`Self::align`].
    pub fn align_mut(&mut self, align_x: Alignment, align_y: Alignment, space: Size) {
        self.bounds.x += alignment_offset(align_x, space.width - self.bounds.width);
        self.bounds.y += alignment_offset(align_y, space.height - self.bounds.height);
    }

    /// Moves the [`Node`] to the given position.
    pub fn move_to(mut self, position: impl Into<Point>) -> Self {
        self.move_to_mut(position);
        self
    }

    /// Mutable reference version of [`Self::move_to`].
    pub fn move_to_mut(&mut self, position: impl Into<Point>) {
        let position = position.into();

        self.bounds.x = position.x;
        self.bounds.y = position.y;
    }

    /// Translates the [`Node`] by the given translation.
    pub fn translate(mut self, translation: impl Into<Vector>) -> Self {
        self.translate_mut(translation);
        self
    }

    /// Translates the [`Node`] by the given translation.
    pub fn translate_mut(&mut self, translation: impl Into<Vector>) {
        self.bounds += translation.into();
    }

    #[inline]
    fn effects_mut(&mut self) -> &mut Effects {
        self.effects
            .get_or_insert_with(|| Box::new(Effects::default()))
    }
}

#[inline]
fn alignment_offset(alignment: Alignment, free_space: f32) -> f32 {
    match alignment {
        Alignment::Start => 0.0,
        Alignment::Center => free_space * 0.5,
        Alignment::End => free_space,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_node_has_no_baseline_transform_or_clip() {
        let node = Node::new(Size::new(10.0, 20.0));

        assert_eq!(node.baseline(), None);
        assert_eq!(node.transform(), None);
        assert_eq!(node.clip(), None);
    }

    #[test]
    fn with_baseline_sets_and_preserves_the_value() {
        let node = Node::new(Size::new(10.0, 20.0)).with_baseline(14.0);

        assert_eq!(node.baseline(), Some(14.0));
    }

    #[test]
    fn with_transform_sets_and_preserves_the_value() {
        let transform = Transformation::translate(5.0, 5.0);
        let node = Node::new(Size::new(10.0, 20.0)).with_transform(transform);

        assert_eq!(node.transform(), Some(transform));
    }

    #[test]
    fn with_clip_sets_and_preserves_the_value() {
        let clip = Rectangle::new(Point::ORIGIN, Size::new(5.0, 5.0));
        let node = Node::new(Size::new(10.0, 20.0)).with_clip(clip);

        assert_eq!(node.clip(), Some(clip));
    }

    #[test]
    fn move_to_and_translate_do_not_disturb_baseline_transform_or_clip() {
        let clip = Rectangle::new(Point::ORIGIN, Size::new(5.0, 5.0));
        let transform = Transformation::translate(1.0, 1.0);

        let node = Node::new(Size::new(10.0, 20.0))
            .with_baseline(14.0)
            .with_transform(transform)
            .with_clip(clip)
            .move_to(Point::new(3.0, 4.0))
            .translate(Vector::new(1.0, 1.0));

        assert_eq!(node.baseline(), Some(14.0));
        assert_eq!(node.transform(), Some(transform));
        assert_eq!(node.clip(), Some(clip));
        assert_eq!(node.bounds().position(), Point::new(4.0, 5.0));
    }

    #[test]
    fn default_node_matches_a_fresh_zero_sized_node() {
        assert_eq!(Node::default(), Node::new(Size::ZERO));
    }

    #[test]
    fn ordinary_nodes_keep_paint_effect_storage_cold() {
        let node = Node::new(Size::new(10.0, 20.0)).with_baseline(14.0);

        assert!(node.effects.is_none());
        assert!(std::mem::size_of::<Node>() <= 64);
    }

    #[test]
    fn transform_and_clip_share_one_cold_allocation() {
        let transform = Transformation::translate(2.0, 3.0);
        let clip = Rectangle::new(Point::ORIGIN, Size::new(4.0, 5.0));
        let node = Node::new(Size::new(10.0, 20.0))
            .with_transform(transform)
            .with_clip(clip);

        assert_eq!(node.transform(), Some(transform));
        assert_eq!(node.clip(), Some(clip));
        assert!(node.effects.is_some());
    }

    #[test]
    fn alignment_offsets_share_the_same_axis_policy() {
        assert_eq!(alignment_offset(Alignment::Start, 8.0), 0.0);
        assert_eq!(alignment_offset(Alignment::Center, 8.0), 4.0);
        assert_eq!(alignment_offset(Alignment::End, 8.0), 8.0);
    }
}
