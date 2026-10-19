//! Typed identity of a split divider in TAIM workspaces.

/// A divider ID owned by the pane-grid tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Split(pub(super) usize);

impl Split {
    /// Local workspace ID useful for debugging and layout trace collection.
    /// A split ID cannot substitute for a Pane, even if both have the same
    /// numeric value.
    #[inline]
    pub const fn local_id(self) -> usize {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_identity_retains_allocator_value() {
        assert_eq!(Split(7).local_id(), 7);
        assert_ne!(Split(7), Split(8));
    }
}
