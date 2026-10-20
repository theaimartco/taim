use super::*;

use smallvec::SmallVec;

/// Computes the flex layout with the given axis and limits, applying spacing,
/// padding and alignment to the items as needed.
///
/// It returns a new layout [`Node`].
pub fn resolve<Message, Theme, Renderer>(
    axis: Axis,
    renderer: &Renderer,
    limits: &Limits,
    width: Length,
    height: Length,
    padding: Padding,
    spacing: f32,
    align_items: Alignment,
    items: &mut [Element<'_, Message, Theme, Renderer>],
    trees: &mut [widget::Tree],
) -> Node
where
    Renderer: crate::Renderer,
{
    let limits = limits.width(width).height(height).shrink(padding);
    let total_spacing = spacing * items.len().saturating_sub(1) as f32;
    let max_cross = axis.cross(limits.max());

    let (main_compress, cross_compress) = {
        let compression = limits.compression();
        axis.pack(compression.width, compression.height)
    };

    let compression = {
        let (compress_x, compress_y) = axis.pack(main_compress, false);
        Size::new(compress_x, compress_y)
    };

    let mut some_fill_cross = false;
    let mut cross = 0.0;
    let mut available = axis.main(limits.max()) - total_spacing;

    let mut nodes: SmallVec<[Option<Node>; INLINE_CHILDREN]> =
        smallvec::smallvec![None; items.len()];

    #[derive(Debug, Clone, Copy)]
    struct Meta {
        main: Length,
        cross: Length,
        category: Category,
    }

    #[derive(Debug, Clone, Copy)]
    enum Category {
        Static,
        CrossFluid,
        CrossFluidDeferred(f32),
        MainFluid,
    }

    let mut metas: SmallVec<[Meta; INLINE_CHILDREN]> = SmallVec::with_capacity(items.len());

    // STATIC PASS
    // We lay out non-fluid elements in the main axis.
    // If we need to compress the cross axis, then we skip any of these elements
    // that are also fluid in the cross axis.
    for (i, child) in items.iter_mut().enumerate() {
        let size = child.as_widget().size();
        let (size_main, size_cross) = axis.pack(size.width, size.height);

        let fill_main_factor = size_main.fill_factor();
        let fill_cross_factor = size_cross.fill_factor();
        let main_is_static = main_compress || fill_main_factor == 0;

        let category = match (main_is_static, cross_compress, fill_cross_factor == 0) {
            (true, false, _) | (true, _, true) => Category::Static,
            (true, true, false) => {
                if let Length::Fixed(main) = size_main {
                    available -= main;
                    Category::CrossFluidDeferred(main)
                } else {
                    Category::CrossFluid
                }
            }
            (false, _, _) => Category::MainFluid,
        };

        let meta = Meta {
            main: size_main,
            cross: size_cross,
            category,
        };

        metas.push(meta);

        let Category::Static = meta.category else {
            some_fill_cross = some_fill_cross || fill_cross_factor != 0;
            continue;
        };

        let (max_width, max_height) = axis.pack(
            available,
            if !cross_compress || fill_cross_factor == 0 {
                max_cross
            } else {
                cross
            },
        );

        let child_limits =
            Limits::with_compression(Size::ZERO, Size::new(max_width, max_height), compression);

        let layout = child
            .as_widget_mut()
            .layout(&mut trees[i], renderer, &child_limits);

        let size = layout.size();

        available -= axis.main(size);
        cross = cross.max(axis.cross(size));
        nodes[i] = Some(layout);
    }

    // CROSS FLUID PASS
    // If we must compress the cross axis and there are fluid elements in the
    // cross axis, we lay out any of these elements that are also non-fluid in
    // the main axis (i.e. the ones we deliberately skipped in the first pass).
    //
    // We use the maximum cross length obtained in the first pass as the maximum
    // cross limit.
    //
    // We can defer the layout of any elements that have a fixed size in the main axis,
    // allowing them to use the cross calculations of the next pass.
    if cross_compress && some_fill_cross {
        for (i, child) in items.iter_mut().enumerate() {
            let meta = metas[i];

            let Category::CrossFluid = meta.category else {
                continue;
            };

            let (max_width, max_height) =
                axis.pack(available, if cross_compress { cross } else { max_cross });

            let child_limits =
                Limits::with_compression(Size::ZERO, Size::new(max_width, max_height), compression);

            let layout = child
                .as_widget_mut()
                .layout(&mut trees[i], renderer, &child_limits);

            let size = layout.size();

            available -= axis.main(size);
            cross = cross.max(axis.cross(size));
            nodes[i] = Some(layout);
        }
    }

    // TAIM WEIGHTED-SPACE PASS
    //
    // The inherited recursive min/max passes depended on iteration order
    // under tight constraints. TAIM first measures intrinsic caps, then
    // allocates the remaining space using a pure bounded-waterfill planner.
    if !main_compress {
        let mut requests: Vec<super::allocator::Request> = metas
            .iter()
            .map(|meta| match meta.category {
                Category::MainFluid => super::allocator::Request::from_length(meta.main),
                _ => super::allocator::Request::from_length(Length::Fit),
            })
            .collect();

        let initial_grants = super::allocator::allocate(available, &requests);
        let mut remaining = available.max(0.0);

        // Children capped by their intrinsic width/height are measured
        // at most once. Unused space is returned to the remaining pool.
        for (i, child) in items.iter_mut().enumerate() {
            if !requests[i].intrinsic_cap() {
                continue;
            }

            let offer = initial_grants[i];
            let (max_width, max_height) = axis.pack(
                offer,
                if !cross_compress || metas[i].cross.fill_factor() == 0 {
                    max_cross
                } else {
                    cross
                },
            );
            let child_limits = Limits::with_compression(
                Size::ZERO,
                Size::new(max_width, max_height),
                compression,
            );

            let node = child
                .as_widget_mut()
                .layout(&mut trees[i], renderer, &child_limits);
            remaining = (remaining - axis.main(node.size())).max(0.0);
            cross = cross.max(axis.cross(node.size()));
            nodes[i] = Some(node);
            requests[i].suppress();
        }

        let grants = super::allocator::allocate(remaining, &requests);
        for (i, child) in items.iter_mut().enumerate() {
            let Category::MainFluid = metas[i].category else {
                continue;
            };
            if nodes[i].is_some() {
                continue;
            }

            let grant = grants[i];
            let min_main = if grant.is_finite() { grant } else { 0.0 };
            let (min_width, min_height) = axis.pack(min_main, 0.0);
            let (max_width, max_height) = axis.pack(
                grant,
                if !cross_compress || metas[i].cross.fill_factor() == 0 {
                    max_cross
                } else {
                    cross
                },
            );

            let child_limits = Limits::with_compression(
                Size::new(min_width, min_height),
                Size::new(max_width, max_height),
                compression,
            );
            let node = child
                .as_widget_mut()
                .layout(&mut trees[i], renderer, &child_limits);

            cross = cross.max(axis.cross(node.size()));
            nodes[i] = Some(node);
        }
    }

    // CROSS FLUID DEFERRED PASS (conditional)
    // We lay out any elements that were deferred in the second pass.
    // These are elements that must be compressed in their cross axis and have
    // a fixed length in the main axis.
    if cross_compress && some_fill_cross {
        for (i, child) in items.iter_mut().enumerate() {
            let meta = metas[i];

            let Category::CrossFluidDeferred(main) = meta.category else {
                continue;
            };

            let (max_width, max_height) = axis.pack(main, cross);
            let child_limits = Limits::new(Size::ZERO, Size::new(max_width, max_height));

            let layout = child
                .as_widget_mut()
                .layout(&mut trees[i], renderer, &child_limits);

            let size = layout.size();

            cross = cross.max(axis.cross(size));
            nodes[i] = Some(layout);
        }
    }

    let pad = axis.pack(padding.left, padding.top);
    let mut main = pad.0;

    let cross = match axis {
        Axis::Horizontal => limits.resolve_height(height, cross),
        Axis::Vertical => limits.resolve_width(width, cross),
    };

    // ALIGNMENT PASS
    // We align all the laid out nodes in the cross axis, if needed.
    for (i, node) in nodes.iter_mut().enumerate() {
        let node = node.as_mut().expect("every node must be resolved");

        if i > 0 {
            main += spacing;
        }

        node.move_to_mut(axis.pack(main, pad.1));

        match axis {
            Axis::Horizontal => {
                node.align_mut(Alignment::Start, align_items, Size::new(0.0, cross));
            }
            Axis::Vertical => {
                node.align_mut(align_items, Alignment::Start, Size::new(cross, 0.0));
            }
        }

        main += axis.main(node.size());
    }

    let main = match axis {
        Axis::Horizontal => limits.resolve_width(width, main - pad.0),
        Axis::Vertical => limits.resolve_height(height, main - pad.0),
    };

    let resolved_nodes: Vec<Node> = nodes
        .into_iter()
        .map(|node| node.expect("every node must be resolved"))
        .collect();

    let size = Size::from(axis.pack(main, cross));
    let baseline = resolve_baseline(axis, &resolved_nodes);

    let node = Node::with_children(size.expand(padding), resolved_nodes);

    match baseline {
        Some(baseline) => node.with_baseline(baseline + padding.top),
        None => node,
    }
}

/// Computes the first-baseline offset (from the top of the produced
/// [`Node`], before padding is added back in) of a flex container from
/// its already-positioned children.
///
/// - On the [`Axis::Horizontal`] (a row), children sit side by side on
///   the same line, so the container's baseline is the highest of its
///   children's own baselines -- the same rule used to align text runs
///   of different sizes on one typographic line.
/// - On the [`Axis::Vertical`] (a column), children stack top to bottom,
///   so only the first child can contribute to a *first*-baseline: any
///   baseline further down is not reachable without laying out content
///   above it first.
///
/// Returns `None` when no child reports a baseline of its own (the
/// common case for non-text content), so a container of only
/// baseline-less children is itself reported as baseline-less rather
/// than defaulting to some arbitrary line.
fn resolve_baseline(axis: Axis, nodes: &[Node]) -> Option<f32> {
    match axis {
        Axis::Horizontal => nodes
            .iter()
            .filter_map(|node| Some(node.bounds().y + node.baseline()?))
            .fold(None, |acc, baseline| {
                Some(acc.map_or(baseline, |acc: f32| acc.max(baseline)))
            }),
        Axis::Vertical => {
            let node = nodes.first()?;

            Some(node.bounds().y + node.baseline()?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Point;

    #[test]
    fn no_children_have_no_baseline() {
        assert_eq!(resolve_baseline(Axis::Horizontal, &[]), None);
        assert_eq!(resolve_baseline(Axis::Vertical, &[]), None);
    }

    #[test]
    fn children_without_a_baseline_produce_no_container_baseline() {
        let nodes = [
            Node::new(Size::new(10.0, 10.0)),
            Node::new(Size::new(10.0, 10.0)),
        ];

        assert_eq!(resolve_baseline(Axis::Horizontal, &nodes), None);
        assert_eq!(resolve_baseline(Axis::Vertical, &nodes), None);
    }

    #[test]
    fn horizontal_baseline_is_the_max_across_children() {
        let nodes = [
            Node::new(Size::new(10.0, 20.0))
                .with_baseline(15.0)
                .move_to(Point::new(0.0, 0.0)),
            Node::new(Size::new(10.0, 30.0))
                .with_baseline(25.0)
                .move_to(Point::new(10.0, 0.0)),
        ];

        assert_eq!(resolve_baseline(Axis::Horizontal, &nodes), Some(25.0));
    }

    #[test]
    fn horizontal_baseline_accounts_for_child_vertical_position() {
        let nodes = [
            Node::new(Size::new(10.0, 20.0))
                .with_baseline(15.0)
                .move_to(Point::new(0.0, 5.0)),
            Node::new(Size::new(10.0, 20.0))
                .with_baseline(10.0)
                .move_to(Point::new(10.0, 0.0)),
        ];

        // First child: 5.0 (y) + 15.0 (baseline) = 20.0
        // Second child: 0.0 (y) + 10.0 (baseline) = 10.0
        assert_eq!(resolve_baseline(Axis::Horizontal, &nodes), Some(20.0));
    }

    #[test]
    fn vertical_baseline_only_considers_the_first_child() {
        let nodes = [
            Node::new(Size::new(10.0, 20.0))
                .with_baseline(15.0)
                .move_to(Point::new(0.0, 0.0)),
            Node::new(Size::new(10.0, 30.0))
                .with_baseline(25.0)
                .move_to(Point::new(0.0, 20.0)),
        ];

        assert_eq!(resolve_baseline(Axis::Vertical, &nodes), Some(15.0));
    }

    #[test]
    fn vertical_baseline_is_none_when_the_first_child_has_none_even_if_others_do() {
        let nodes = [
            Node::new(Size::new(10.0, 20.0)).move_to(Point::new(0.0, 0.0)),
            Node::new(Size::new(10.0, 30.0))
                .with_baseline(25.0)
                .move_to(Point::new(0.0, 20.0)),
        ];

        assert_eq!(resolve_baseline(Axis::Vertical, &nodes), None);
    }
}
