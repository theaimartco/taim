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
    // Direct callers may bypass the Row/Column spacing setters.
    let spacing = if spacing.is_finite() { spacing.max(0.0) } else { 0.0 };
    let max_cross = axis.cross(limits.max());

    let (main_compress, cross_compress) = {
        let compression = limits.compression();
        axis.pack(compression.width, compression.height)
    };

    let compression = {
        let (compress_x, compress_y) = axis.pack(main_compress, false);
        Size::new(compress_x, compress_y)
    };

    // TAIM classifies children once, before performing any layout. In
    // particular, fixed-size cross-dependent children reserve room up
    // front instead of stealing it based on their sibling order.
    let schedule = super::plan::Schedule::build(
        items.iter().map(|child| {
            let size = child.as_widget().size();
            axis.pack(size.width, size.height)
        }),
        main_compress,
        cross_compress,
    );
    let mut available = schedule.main_budget(axis.main(limits.max()), spacing);
    let mut cross = 0.0;

    let mut nodes: SmallVec<[Option<Node>; INLINE_CHILDREN]> =
        smallvec::smallvec![None; items.len()];

    // Intrinsic sizing is measured first; cross-dependent children are
    // probed only after the cross span has been discovered.
    for (i, child) in items.iter_mut().enumerate() {
        let slot = schedule.slots[i];
        if slot.pass != super::plan::Pass::Intrinsic {
            continue;
        }

        let (max_width, max_height) = axis.pack(
            available,
            if !cross_compress || slot.cross.fill_factor() == 0 {
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
        available = (available - axis.main(node.size())).max(0.0);
        cross = cross.max(axis.cross(node.size()));
        nodes[i] = Some(node);
    }

    if cross_compress && schedule.has_cross_dependents {
        for (i, child) in items.iter_mut().enumerate() {
            if schedule.slots[i].pass != super::plan::Pass::CrossProbe {
                continue;
            }

            let (max_width, max_height) = axis.pack(available, cross);
            let child_limits = Limits::with_compression(
                Size::ZERO,
                Size::new(max_width, max_height),
                compression,
            );
            let node = child
                .as_widget_mut()
                .layout(&mut trees[i], renderer, &child_limits);
            available = (available - axis.main(node.size())).max(0.0);
            cross = cross.max(axis.cross(node.size()));
            nodes[i] = Some(node);
        }
    }

    // TAIM WEIGHTED-SPACE PASS
    //
    // The inherited recursive min/max passes depended on iteration order
    // under tight constraints. TAIM first measures intrinsic caps, then
    // allocates the remaining space using a pure bounded-waterfill planner.
    if !main_compress {
        let mut requests: Vec<super::allocator::Request> = schedule.slots
            .iter()
            .map(|meta| match meta.pass {
                super::plan::Pass::Weighted => super::allocator::Request::from_length(meta.main),
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
                if !cross_compress || schedule.slots[i].cross.fill_factor() == 0 {
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
            let super::plan::Pass::Weighted = schedule.slots[i].pass else {
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
                if !cross_compress || schedule.slots[i].cross.fill_factor() == 0 {
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

    // Last, use the settled cross span for fixed-main cross-flexible children.
    if cross_compress && schedule.has_cross_dependents {
        for (i, child) in items.iter_mut().enumerate() {
            let super::plan::Pass::CrossDeferred(main) = schedule.slots[i].pass else {
                continue;
            };
            let (max_width, max_height) = axis.pack(main, cross);
            let child_limits = Limits::new(Size::ZERO, Size::new(max_width, max_height));
            let node = child
                .as_widget_mut()
                .layout(&mut trees[i], renderer, &child_limits);
            cross = cross.max(axis.cross(node.size()));
            nodes[i] = Some(node);
        }
    }

    // The layout engine measures children. TAIM's separate placement engine
    // computes stable offsets, cross-axis alignment, the first baseline and
    // the resolved container size in one deterministic finishing pass.
    let children = nodes
        .into_iter()
        .map(|node| node.expect("TAIM flex schedule must measure every child"))
        .collect();

    super::placement::Frame {
        axis,
        limits: &limits,
        width,
        height,
        padding,
        spacing,
        alignment: align_items,
        cross_extent: cross,
    }
    .finish(children)
}
