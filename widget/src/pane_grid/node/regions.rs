use super::Node;
use crate::core::{Rectangle, Size};
use crate::pane_grid::{Axis, Pane, Split};

use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy)]
struct Count {
    horizontal: usize,
    vertical: usize,
    children: Option<(usize, usize)>,
}

impl Node {
    fn count_into(&self, counts: &mut Vec<Count>) -> usize {
        match self {
            Self::Split { a, b, axis, .. } => {
                let a = a.count_into(counts);
                let b = b.count_into(counts);

                let (horizontal, vertical) = match axis {
                    Axis::Horizontal => (
                        1 + counts[a].horizontal + counts[b].horizontal,
                        counts[a].vertical.max(counts[b].vertical),
                    ),
                    Axis::Vertical => (
                        counts[a].horizontal.max(counts[b].horizontal),
                        1 + counts[a].vertical + counts[b].vertical,
                    ),
                };

                counts.push(Count {
                    horizontal,
                    vertical,
                    children: Some((a, b)),
                });
            }
            Self::Pane(_) => counts.push(Count {
                horizontal: 0,
                vertical: 0,
                children: None,
            }),
        }

        counts.len() - 1
    }

    /// Returns the rectangular region for each [`Pane`] in the [`Node`] given
    /// the spacing between panes and the total available space.
    pub fn pane_regions(
        &self,
        spacing: f32,
        min_size: f32,
        bounds: Size,
    ) -> BTreeMap<Pane, Rectangle> {
        let spacing = finite_nonnegative(spacing);
        let min_size = finite_nonnegative(min_size);
        let bounds = finite_size(bounds);
        let mut regions = BTreeMap::new();
        let mut counts = Vec::new();
        let root = self.count_into(&mut counts);

        self.compute_regions(
            spacing,
            min_size,
            &Rectangle::new([0.0, 0.0].into(), bounds),
            root,
            &counts,
            &mut regions,
        );

        regions
    }

    /// Returns the axis, rectangular region, and ratio for each [`Split`] in
    /// the [`Node`] given the spacing between panes and the total available
    /// space.
    pub fn split_regions(
        &self,
        spacing: f32,
        min_size: f32,
        bounds: Size,
    ) -> BTreeMap<Split, (Axis, Rectangle, f32)> {
        let spacing = finite_nonnegative(spacing);
        let min_size = finite_nonnegative(min_size);
        let bounds = finite_size(bounds);
        let mut splits = BTreeMap::new();
        let mut counts = Vec::new();
        let root = self.count_into(&mut counts);

        self.compute_splits(
            spacing,
            min_size,
            &Rectangle::new([0.0, 0.0].into(), bounds),
            root,
            &counts,
            &mut splits,
        );

        splits
    }

    fn compute_regions(
        &self,
        spacing: f32,
        min_size: f32,
        current: &Rectangle,
        count: usize,
        counts: &[Count],
        regions: &mut BTreeMap<Pane, Rectangle>,
    ) {
        match self {
            Self::Split {
                axis, ratio, a, b, ..
            } => {
                let Some((count_a, count_b)) = counts[count].children else {
                    unreachable!("split node must have split counts")
                };

                let (a_factor, b_factor) = match axis {
                    Axis::Horizontal => (counts[count_a].horizontal, counts[count_b].horizontal),
                    Axis::Vertical => (counts[count_a].vertical, counts[count_b].vertical),
                };

                let (region_a, region_b, _) = axis.split(
                    current,
                    *ratio,
                    spacing,
                    minimum_extent(min_size, spacing, a_factor),
                    minimum_extent(min_size, spacing, b_factor),
                );

                a.compute_regions(spacing, min_size, &region_a, count_a, counts, regions);
                b.compute_regions(spacing, min_size, &region_b, count_b, counts, regions);
            }
            Self::Pane(pane) => {
                debug_assert!(counts[count].children.is_none());
                let _ = regions.insert(*pane, *current);
            }
        }
    }

    fn compute_splits(
        &self,
        spacing: f32,
        min_size: f32,
        current: &Rectangle,
        count: usize,
        counts: &[Count],
        splits: &mut BTreeMap<Split, (Axis, Rectangle, f32)>,
    ) {
        match self {
            Self::Split {
                axis,
                ratio,
                a,
                b,
                id,
            } => {
                let Some((count_a, count_b)) = counts[count].children else {
                    unreachable!("split node must have split counts")
                };

                let (a_factor, b_factor) = match axis {
                    Axis::Horizontal => (counts[count_a].horizontal, counts[count_b].horizontal),
                    Axis::Vertical => (counts[count_a].vertical, counts[count_b].vertical),
                };

                let (region_a, region_b, ratio) = axis.split(
                    current,
                    *ratio,
                    spacing,
                    minimum_extent(min_size, spacing, a_factor),
                    minimum_extent(min_size, spacing, b_factor),
                );

                let _ = splits.insert(*id, (*axis, *current, ratio));

                a.compute_splits(spacing, min_size, &region_a, count_a, counts, splits);
                b.compute_splits(spacing, min_size, &region_b, count_b, counts, splits);
            }
            Self::Pane(_) => {
                debug_assert!(counts[count].children.is_none());
            }
        }
    }
}

#[inline]
fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[inline]
fn finite_size(size: Size) -> Size {
    Size::new(
        finite_nonnegative(size.width),
        finite_nonnegative(size.height),
    )
}

#[inline]
fn minimum_extent(min_size: f32, spacing: f32, separators: usize) -> f32 {
    let separators = separators as f64;
    let extent = f64::from(min_size) * (separators + 1.0) + f64::from(spacing) * separators;

    extent.min(f64::from(f32::MAX)) as f32
}
