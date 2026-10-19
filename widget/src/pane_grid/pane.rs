//! Typed, noninterchangeable IDs for TAIM workspace panes.
//!
//! Session storage can read an ID for diagnostics without accessing the
//! internal allocator or accidentally treating it as a split handle.

/// Identity of one rectangular pane in a split workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pane(pub(super) usize);

impl Pane {
    /// Stable within a single living workspace; NOT globally unique.
    #[inline]
    pub const fn local_id(self) -> usize {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exposes_stable_local_id_without_allocating() {
        let pane = Pane(12);
        assert_eq!(pane.local_id(), 12);
        assert_eq!(pane, Pane(12));
        assert_ne!(pane, Pane(13));
    }
}
