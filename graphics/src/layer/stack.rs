use super::{Layer, Stack};

use crate::core::{Rectangle, Transformation};

use std::ops::Range;

impl<T: Layer> Stack<T> {
    /// Creates an empty recording stack with one reusable base layer.
    pub fn new() -> Self {
        Self {
            layers: vec![T::default()],
            transformations: vec![Transformation::IDENTITY],
            opacities: vec![1.0],
            previous: Vec::new(),
            current: 0,
            active_count: 1,
        }
    }

    /// Returns the active layer and cumulative transformation.
    #[inline]
    pub fn current_mut(&mut self) -> (&mut T, Transformation) {
        let transformation = self.transformation();
        (&mut self.layers[self.current], transformation)
    }

    /// Returns the cumulative transformation at the top of the stack.
    #[inline]
    pub fn transformation(&self) -> Transformation {
        self.transformations
            .last()
            .copied()
            .unwrap_or(Transformation::IDENTITY)
    }

    /// Returns the cumulative opacity at the top of the stack.
    #[inline]
    pub fn opacity(&self) -> f32 {
        self.opacities.last().copied().unwrap_or(1.0)
    }

    /// Pushes a finite, normalized opacity multiplier.
    pub fn push_opacity(&mut self, opacity: f32) {
        let opacity = if opacity.is_finite() {
            opacity.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.opacities.push(self.opacity() * opacity);
    }

    /// Pushes a finite, normalized opacity that replaces the inherited one.
    pub fn push_opacity_reset(&mut self, opacity: f32) {
        let opacity = if opacity.is_finite() {
            opacity.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.opacities.push(opacity);
    }

    /// Pops an opacity multiplier without removing the base value.
    pub fn pop_opacity(&mut self) {
        if self.opacities.len() > 1 {
            let _ = self.opacities.pop();
        }
    }

    /// Starts a layer clipped to both `bounds` and its parent clip.
    pub fn push_clip(&mut self, bounds: Rectangle) {
        let parent = self.layers[self.current].bounds();
        let transformed = bounds * self.transformation();
        let clipped = transformed.intersection(&parent).unwrap_or(Rectangle {
            width: 0.0,
            height: 0.0,
            ..transformed
        });

        self.previous.push(self.current);
        self.current = self.active_count;
        self.active_count += 1;

        if self.current == self.layers.len() {
            self.layers.push(T::with_bounds(clipped));
        } else {
            self.layers[self.current].resize(clipped);
        }
    }

    /// Finishes the active clip layer and restores its parent.
    pub fn pop_clip(&mut self) {
        if self.previous.is_empty() {
            return;
        }

        self.flush();
        self.current = self.previous.pop().unwrap_or(0);
    }

    /// Pushes a transformation composed with the current transformation.
    pub fn push_transformation(&mut self, transformation: Transformation) {
        self.transformations
            .push(self.transformation() * transformation);
    }

    /// Pops a transformation without removing the identity base value.
    pub fn pop_transformation(&mut self) {
        if self.transformations.len() > 1 {
            let _ = self.transformations.pop();
        }
    }

    /// Iterates over layers recorded in the current frame.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.as_slice().iter()
    }

    /// Returns layers recorded in the current frame.
    pub fn as_slice(&self) -> &[T] {
        &self.layers[..self.active_count]
    }

    /// Returns the number of active layers.
    pub fn active_count(&self) -> usize {
        self.active_count
    }

    /// Extracts one recorded range and resets the stack for a new frame.
    pub fn take_range(&mut self, range: Range<usize>, new_bounds: Rectangle) -> Self {
        let recorded = std::mem::take(self);
        self.reset(new_bounds);

        let start = range.start.min(recorded.active_count);
        let end = range.end.min(recorded.active_count).max(start);
        let layers = recorded
            .layers
            .into_iter()
            .skip(start)
            .take(end - start)
            .collect();

        Self::display_list(layers)
    }

    /// Extracts recorded ranges and resets the stack for a new frame.
    pub fn take_ranges(&mut self, ranges: &[Range<usize>], new_bounds: Rectangle) -> Vec<Self> {
        let recorded = std::mem::take(self);
        self.reset(new_bounds);

        if ranges.windows(2).all(|pair| pair[0].end <= pair[1].start) {
            return Self::take_ordered_ranges(recorded, ranges);
        }

        Self::take_overlapping_ranges(recorded, ranges)
    }

    fn take_ordered_ranges(recorded: Self, ranges: &[Range<usize>]) -> Vec<Self> {
        let active_count = recorded.active_count;
        let mut source = recorded
            .layers
            .into_iter()
            .take(active_count)
            .enumerate()
            .peekable();
        let mut lists = Vec::with_capacity(ranges.len());

        for range in ranges {
            let start = range.start.min(active_count);
            let end = range.end.min(active_count).max(start);

            while source.peek().is_some_and(|(index, _)| *index < start) {
                let _ = source.next();
            }

            let mut layers = Vec::with_capacity(end - start);
            while source.peek().is_some_and(|(index, _)| *index < end) {
                if let Some((_, layer)) = source.next() {
                    layers.push(layer);
                }
            }

            lists.push(Self::display_list(layers));
        }

        lists
    }

    fn take_overlapping_ranges(recorded: Self, ranges: &[Range<usize>]) -> Vec<Self> {
        let active_count = recorded.active_count;
        let mut layers = recorded
            .layers
            .into_iter()
            .take(active_count)
            .map(Some)
            .collect::<Vec<_>>();

        ranges
            .iter()
            .map(|range| {
                let start = range.start.min(active_count);
                let end = range.end.min(active_count).max(start);
                let selected = layers[start..end]
                    .iter_mut()
                    .filter_map(Option::take)
                    .collect();
                Self::display_list(selected)
            })
            .collect()
    }

    fn display_list(layers: Vec<T>) -> Self {
        let active_count = layers.len();

        Self {
            layers,
            transformations: vec![Transformation::IDENTITY],
            opacities: vec![1.0],
            previous: Vec::new(),
            current: 0,
            active_count,
        }
    }

    /// Replays an owned display list and returns its new active range.
    pub fn replay_owned(&mut self, mut display_list: Self) -> Range<usize> {
        let start = self.active_count;
        let count = display_list.active_count;
        let layers = std::mem::take(&mut display_list.layers);

        for layer in layers.into_iter().take(count) {
            if self.active_count == self.layers.len() {
                self.layers.push(layer);
            } else {
                self.layers[self.active_count] = layer;
            }
            self.active_count += 1;
        }

        start..self.active_count
    }

    /// Estimates bytes retained by all active layers.
    pub fn estimated_bytes(&self) -> usize {
        self.layers[..self.active_count]
            .iter()
            .fold(std::mem::size_of::<Self>(), |total, layer| {
                total.saturating_add(layer.estimated_bytes().max(4 * 1024))
            })
    }

    /// Settles pending primitives in the active layer.
    pub fn flush(&mut self) {
        if let Some(layer) = self.layers.get_mut(self.current) {
            layer.flush();
        }
    }

    /// Merges compatible adjacent layers where ordering permits.
    pub fn merge(&mut self) {
        self.flush();
        let mut left = self.active_count;

        while left > 1 {
            let mut current = left - 1;
            let mut target = &self.layers[current];
            let mut target_start = target.start();
            let mut target_index = current;

            while current > 0 {
                current -= 1;

                let candidate = &self.layers[current];
                let start = candidate.start();
                let end = candidate.end();

                if end == 0 {
                    continue;
                }

                if end > target_start || candidate.bounds() != target.bounds() {
                    break;
                }

                target = candidate;
                target_start = start;
                target_index = current;
            }

            let (head, tail) = self.layers.split_at_mut(target_index + 1);
            let layer = &mut head[target_index];

            for middle in &mut tail[..left - target_index - 1] {
                layer.merge(middle);
            }

            left = current;
        }
    }

    /// Clears active layers while retaining their allocations for reuse.
    pub fn reset(&mut self, new_bounds: Rectangle) {
        for layer in self.layers[..self.active_count].iter_mut() {
            layer.reset();
        }

        if self.layers.is_empty() {
            self.layers.push(T::with_bounds(new_bounds));
        } else {
            self.layers[0].resize(new_bounds);
        }

        self.current = 0;
        self.active_count = 1;
        self.previous.clear();

        self.transformations.truncate(1);
        if self.transformations.is_empty() {
            self.transformations.push(Transformation::IDENTITY);
        } else {
            self.transformations[0] = Transformation::IDENTITY;
        }

        self.opacities.truncate(1);
        if self.opacities.is_empty() {
            self.opacities.push(1.0);
        } else {
            self.opacities[0] = 1.0;
        }
    }
}

impl<T: Layer> Default for Stack<T> {
    fn default() -> Self {
        Self::new()
    }
}
