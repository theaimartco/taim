//! Draw and stack layers of graphical primitives.
use crate::core::{Rectangle, Transformation};

/// A layer of graphical primitives.
///
/// Layers normally dictate a set of primitives that are
/// rendered in a specific order.
pub trait Layer: Default {
    /// Creates a new [`Layer`] with the given bounds.
    fn with_bounds(bounds: Rectangle) -> Self;

    /// Returns the current bounds of the [`Layer`].
    fn bounds(&self) -> Rectangle;

    /// Flushes and settles any pending group of primitives in the [`Layer`].
    ///
    /// This will be called when a [`Layer`] is finished. It allows layers to efficiently
    /// record primitives together and defer grouping until the end.
    fn flush(&mut self);

    /// Resizes the [`Layer`] to the given bounds.
    fn resize(&mut self, bounds: Rectangle);

    /// Clears all the layers contents and resets its bounds.
    fn reset(&mut self);

    /// Returns the start level of the [`Layer`].
    ///
    /// A level is a "sublayer" index inside of a [`Layer`].
    ///
    /// A [`Layer`] may draw multiple primitive types in a certain order.
    /// The level represents the lowest index of the primitive types it
    /// contains.
    ///
    /// Two layers A and B can therefore be merged if they have the same bounds,
    /// and the end level of A is lower or equal than the start level of B.
    fn start(&self) -> usize;

    /// Returns the end level of the [`Layer`].
    fn end(&self) -> usize;

    /// Merges a [`Layer`] with the current one.
    fn merge(&mut self, _layer: &mut Self);

    /// Estimates the owned bytes retained by this layer.
    ///
    /// Backends may override this with batch-capacity accounting. The default
    /// still charges the layer object itself, so every cache entry has a
    /// positive bounded cost.
    fn estimated_bytes(&self) -> usize {
        std::mem::size_of_val(self)
    }
}

/// A stack of layers used for drawing.
#[derive(Debug)]
pub struct Stack<T: Layer> {
    layers: Vec<T>,
    transformations: Vec<Transformation>,
    opacities: Vec<f32>,
    previous: Vec<usize>,
    current: usize,
    active_count: usize,
}

impl<T: Layer> Stack<T> {
    /// Creates a new empty [`Stack`].
    pub fn new() -> Self {
        Self {
            layers: vec![T::default()],
            transformations: vec![Transformation::IDENTITY],
            opacities: vec![1.0],
            previous: vec![],
            current: 0,
            active_count: 1,
        }
    }

    /// Returns a mutable reference to the current [`Layer`] of the [`Stack`], together with
    /// the current [`Transformation`].
    #[inline]
    pub fn current_mut(&mut self) -> (&mut T, Transformation) {
        let transformation = self.transformation();

        (&mut self.layers[self.current], transformation)
    }

    /// Returns the current [`Transformation`] of the [`Stack`].
    #[inline]
    pub fn transformation(&self) -> Transformation {
        self.transformations.last().copied().unwrap()
    }

    /// Returns the current cumulative opacity multiplier.
    pub fn opacity(&self) -> f32 {
        self.opacities.last().copied().unwrap_or(1.0)
    }

    /// Pushes a nested opacity multiplier.
    pub fn push_opacity(&mut self, opacity: f32) {
        self.opacities
            .push(self.opacity() * opacity.clamp(0.0, 1.0));
    }

    /// Pops the current opacity multiplier.
    pub fn pop_opacity(&mut self) {
        if self.opacities.len() > 1 {
            let _ = self.opacities.pop();
        }
    }

    /// Pushes a new clipping region in the [`Stack`]; creating a new layer in the
    /// process.
    pub fn push_clip(&mut self, bounds: Rectangle) {
        self.previous.push(self.current);

        self.current = self.active_count;
        self.active_count += 1;

        let bounds = bounds * self.transformation();

        if self.current == self.layers.len() {
            self.layers.push(T::with_bounds(bounds));
        } else {
            self.layers[self.current].resize(bounds);
        }
    }

    /// Pops the current clipping region from the [`Stack`] and restores the previous one.
    ///
    /// The current layer will be recorded for drawing.
    pub fn pop_clip(&mut self) {
        self.flush();

        self.current = self.previous.pop().unwrap();
    }

    /// Pushes a new [`Transformation`] in the [`Stack`].
    ///
    /// Future drawing operations will be affected by this new [`Transformation`] until
    /// it is popped using [`pop_transformation`].
    ///
    /// [`pop_transformation`]: Self::pop_transformation
    pub fn push_transformation(&mut self, transformation: Transformation) {
        self.transformations
            .push(self.transformation() * transformation);
    }

    /// Pops the current [`Transformation`] in the [`Stack`].
    pub fn pop_transformation(&mut self) {
        let _ = self.transformations.pop();
    }

    /// Returns an iterator over immutable references to the layers in the [`Stack`].
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.layers[..self.active_count].iter()
    }

    /// Returns the slice of layers in the [`Stack`].
    pub fn as_slice(&self) -> &[T] {
        &self.layers[..self.active_count]
    }

    /// Returns the number of layers recorded in the current frame.
    pub fn active_count(&self) -> usize {
        self.active_count
    }

    /// Moves a contiguous recorded range into an independent display list
    /// and resets this stack for a new frame.
    pub fn take_range(&mut self, range: std::ops::Range<usize>, new_bounds: Rectangle) -> Self {
        let mut recorded = std::mem::take(self);
        self.reset(new_bounds);

        let start = range.start.min(recorded.active_count);
        let end = range.end.min(recorded.active_count).max(start);
        let layers: Vec<T> = recorded.layers.drain(start..end).collect();
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

    /// Moves multiple disjoint recorded ranges into independent display lists
    /// and resets this stack for a new frame.
    pub fn take_ranges(
        &mut self,
        ranges: &[std::ops::Range<usize>],
        new_bounds: Rectangle,
    ) -> Vec<Self> {
        let recorded = std::mem::take(self);
        self.reset(new_bounds);

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
                let layers = layers[start..end]
                    .iter_mut()
                    .filter_map(Option::take)
                    .collect::<Vec<_>>();
                let active_count = layers.len();

                Self {
                    layers,
                    transformations: vec![Transformation::IDENTITY],
                    opacities: vec![1.0],
                    previous: Vec::new(),
                    current: 0,
                    active_count,
                }
            })
            .collect()
    }

    /// Moves a retained display list into this frame and returns the range
    /// occupied by the replayed layers.
    pub fn replay_owned(&mut self, mut display_list: Self) -> std::ops::Range<usize> {
        let start = self.active_count;
        let count = display_list.active_count;

        for layer in display_list.layers.drain(..count) {
            if self.active_count == self.layers.len() {
                self.layers.push(layer);
            } else {
                self.layers[self.active_count] = layer;
            }
            self.active_count += 1;
        }

        start..self.active_count
    }

    /// Returns the estimated bytes owned by the active display list.
    pub fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.layers[..self.active_count]
                .iter()
                .map(|layer| layer.estimated_bytes().max(4 * 1024))
                .sum::<usize>()
    }

    /// Flushes and settles any primitives in the [`Stack`].
    pub fn flush(&mut self) {
        self.layers[self.current].flush();
    }

    /// Performs layer merging wherever possible.
    ///
    /// Flushes and settles any primitives in the [`Stack`].
    pub fn merge(&mut self) {
        self.flush();

        // These are the layers left to process
        let mut left = self.active_count;

        // There must be at least 2 or more layers to merge
        while left > 1 {
            // We set our target as the topmost layer left to process
            let mut current = left - 1;
            let mut target = &self.layers[current];
            let mut target_start = target.start();
            let mut target_index = current;

            // We scan downwards for a contiguous block of mergeable layer candidates
            while current > 0 {
                current -= 1;

                let candidate = &self.layers[current];
                let start = candidate.start();
                let end = candidate.end();

                // We skip empty layers
                if end == 0 {
                    continue;
                }

                // Candidate can be merged if primitive sublayers do not overlap with
                // previous targets and the clipping bounds match
                if end > target_start || candidate.bounds() != target.bounds() {
                    break;
                }

                // Candidate is not empty and can be merged into
                target = candidate;
                target_start = start;
                target_index = current;
            }

            // We merge all the layers scanned into the target
            //
            // Since we use `target_index` instead of `current`, we
            // deliberately avoid merging into empty layers.
            //
            // If no candidates were mergeable, this is a no-op.
            let (head, tail) = self.layers.split_at_mut(target_index + 1);
            let layer = &mut head[target_index];

            for middle in &mut tail[0..left - target_index - 1] {
                layer.merge(middle);
            }

            // Empty layers found after the target can be skipped
            left = current;
        }
    }

    /// Clears the layers of the [`Stack`], allowing reuse.
    ///
    /// It resizes the base layer bounds to the `new_bounds`.
    ///
    /// This will normally keep layer allocations for future drawing operations.
    pub fn reset(&mut self, new_bounds: Rectangle) {
        for layer in self.layers[..self.active_count].iter_mut() {
            layer.reset();
        }

        self.layers[0].resize(new_bounds);
        self.current = 0;
        self.active_count = 1;
        self.previous.clear();
        self.transformations.truncate(1);
        self.opacities.truncate(1);
        self.opacities[0] = 1.0;
    }
}

impl<T: Layer> Default for Stack<T> {
    fn default() -> Self {
        Self::new()
    }
}
