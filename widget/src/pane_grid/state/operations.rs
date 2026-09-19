use super::State;

use crate::pane_grid::{Axis, Direction, Edge, Pane, Region, Split, Target};

impl<T> State<T> {
    /// Returns the adjacent [`Pane`] of another [`Pane`] in the given direction.
    pub fn adjacent(&self, pane: Pane, direction: Direction) -> Option<Pane> {
        self.internal.layout.adjacent_pane(pane, direction)
    }

    /// Splits a pane and initializes the new pane with `state`.
    pub fn split(&mut self, axis: Axis, pane: Pane, state: T) -> Option<(Pane, Split)> {
        self.split_node(axis, Some(pane), state, false)
    }

    /// Splits `target` with an existing pane at the requested region.
    pub fn split_with(&mut self, target: Pane, pane: Pane, region: Region) {
        match region {
            Region::Center => self.swap(pane, target),
            Region::Edge(Edge::Top) => {
                self.split_and_swap(Axis::Horizontal, target, pane, true);
            }
            Region::Edge(Edge::Bottom) => {
                self.split_and_swap(Axis::Horizontal, target, pane, false);
            }
            Region::Edge(Edge::Left) => {
                self.split_and_swap(Axis::Vertical, target, pane, true);
            }
            Region::Edge(Edge::Right) => {
                self.split_and_swap(Axis::Vertical, target, pane, false);
            }
        }
    }

    /// Drops a pane into a pane or outer edge target.
    pub fn drop(&mut self, pane: Pane, target: Target) {
        match target {
            Target::Edge(edge) => self.move_to_edge(pane, edge),
            Target::Pane(target, region) => self.split_with(target, pane, region),
        }
    }

    fn split_node(
        &mut self,
        axis: Axis,
        pane: Option<Pane>,
        state: T,
        inverse: bool,
    ) -> Option<(Pane, Split)> {
        let node = match pane {
            Some(pane) => self.internal.layout.find(pane)?,
            None => &mut self.internal.layout,
        };
        let pane_id = self.internal.last_id.checked_add(1)?;
        let split_id = pane_id.checked_add(1)?;
        let new_pane = Pane(pane_id);
        let new_split = Split(split_id);

        self.internal.last_id = split_id;

        if inverse {
            node.split_inverse(new_split, axis, new_pane);
        } else {
            node.split(new_split, axis, new_pane);
        }

        let _ = self.panes.insert(new_pane, state);
        self.internal.maximized = None;

        Some((new_pane, new_split))
    }

    fn split_and_swap(&mut self, axis: Axis, target: Pane, pane: Pane, swap: bool) {
        if let Some((state, _)) = self.close(pane)
            && let Some((new_pane, _)) = self.split(axis, target, state)
        {
            self.relabel(new_pane, pane);

            if swap {
                self.swap(target, pane);
            }
        }
    }

    /// Moves a pane to an outer edge of the grid.
    pub fn move_to_edge(&mut self, pane: Pane, edge: Edge) {
        match edge {
            Edge::Top => self.split_major_node_and_swap(Axis::Horizontal, pane, true),
            Edge::Bottom => self.split_major_node_and_swap(Axis::Horizontal, pane, false),
            Edge::Left => self.split_major_node_and_swap(Axis::Vertical, pane, true),
            Edge::Right => self.split_major_node_and_swap(Axis::Vertical, pane, false),
        }
    }

    fn split_major_node_and_swap(&mut self, axis: Axis, pane: Pane, inverse: bool) {
        if let Some((state, _)) = self.close(pane)
            && let Some((new_pane, _)) = self.split_node(axis, None, state, inverse)
        {
            self.relabel(new_pane, pane);
        }
    }

    fn relabel(&mut self, target: Pane, label: Pane) {
        let relabeled = self.internal.layout.relabel(target, label);
        debug_assert!(relabeled, "new pane must exist in the pane layout");

        if let Some(state) = self.panes.remove(&target) {
            let replaced = self.panes.insert(label, state);
            debug_assert!(replaced.is_none(), "pane labels must stay unique");
        }
    }

    /// Swaps the position of two panes while preserving their state keys.
    pub fn swap(&mut self, a: Pane, b: Pane) {
        if a == b || !self.panes.contains_key(&a) || !self.panes.contains_key(&b) {
            return;
        }

        let found = self.internal.layout.swap_panes(a, b);
        debug_assert_eq!(found, 0b11, "pane map and layout must stay synchronized");
    }

    /// Resizes a split using a finite ratio clamped to `[0, 1]`.
    pub fn resize(&mut self, split: Split, ratio: f32) {
        if ratio.is_finite() {
            let _ = self.internal.layout.resize(split, ratio.clamp(0.0, 1.0));
        }
    }

    /// Closes a pane and returns its state and closest sibling.
    pub fn close(&mut self, pane: Pane) -> Option<(T, Pane)> {
        if self.internal.maximized == Some(pane) {
            self.internal.maximized = None;
        }

        let sibling = self.internal.layout.remove(pane)?;
        self.panes.remove(&pane).map(|state| (state, sibling))
    }

    /// Shows only the given pane until [`Self::restore`] is called.
    pub fn maximize(&mut self, pane: Pane) {
        if self.panes.contains_key(&pane) {
            self.internal.maximized = Some(pane);
        }
    }

    /// Restores all panes after maximization.
    pub fn restore(&mut self) {
        self.internal.maximized = None;
    }

    /// Returns the maximized pane, if any.
    pub fn maximized(&self) -> Option<Pane> {
        self.internal.maximized
    }
}
