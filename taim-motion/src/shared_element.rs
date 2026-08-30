//! Shared-element lifecycle bookkeeping.

use alloc::collections::BTreeMap;

/// A stable caller-provided key for a shared element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SharedElementId(pub u64);

/// Renderer-neutral snapshot metadata. The renderer owns the referenced layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snapshot<Layer> {
    /// Opaque retained-layer handle.
    pub layer: Layer,
    /// Logical width at capture time.
    pub width: f32,
    /// Logical height at capture time.
    pub height: f32,
}

/// One registered shared element.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SharedElement<Layer> {
    /// Current renderer snapshot.
    pub snapshot: Snapshot<Layer>,
    /// Generation assigned by the registry; changes invalidate stale cleanups.
    pub generation: u64,
}

/// A bounded registry that guarantees cancel/navigation cleanup releases layers.
#[derive(Debug, Clone)]
pub struct SharedElements<Layer> {
    entries: BTreeMap<SharedElementId, SharedElement<Layer>>,
    capacity: usize,
    generation: u64,
}

impl<Layer: Copy> SharedElements<Layer> {
    /// Creates a registry with a hard entry bound.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            capacity,
            generation: 0,
        }
    }

    /// Registers a snapshot. Returns the replaced layer, or the supplied layer
    /// unchanged if capacity is zero or a new entry would exceed the budget.
    pub fn register(&mut self, id: SharedElementId, snapshot: Snapshot<Layer>) -> Option<Layer> {
        self.generation = self.generation.wrapping_add(1);
        if !self.entries.contains_key(&id) && self.entries.len() >= self.capacity {
            return Some(snapshot.layer);
        }
        self.entries
            .insert(
                id,
                SharedElement {
                    snapshot,
                    generation: self.generation,
                },
            )
            .map(|old| old.snapshot.layer)
    }

    /// Removes one entry and returns its layer for immediate renderer release.
    pub fn remove(&mut self, id: SharedElementId) -> Option<Layer> {
        self.entries.remove(&id).map(|entry| entry.snapshot.layer)
    }

    /// Cancels a transition/navigation and returns every retained layer.
    pub fn clear(&mut self) -> alloc::vec::Vec<Layer> {
        let entries = core::mem::take(&mut self.entries);
        entries
            .into_values()
            .map(|entry| entry.snapshot.layer)
            .collect()
    }

    /// Returns the number of owned snapshots.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether no snapshots are retained.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
