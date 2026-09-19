//! Access the clipboard.
mod model;

#[cfg(feature = "image")]
pub use model::Image;
pub use model::{Content, Error, Event, Kind};

/// A set of clipboard requests.
#[derive(Debug, Clone)]
pub struct Clipboard {
    /// The read requests the runtime must fulfill.
    pub reads: Vec<Kind>,
    /// The content that must be written to the clipboard by the runtime,
    /// if any.
    pub write: Option<Content>,
}

impl Clipboard {
    /// Creates a new empty set of [`Clipboard`] requests.
    pub fn new() -> Self {
        Self {
            reads: Vec::new(),
            write: None,
        }
    }

    /// Creates an empty request set with room for `reads` read requests.
    pub fn with_read_capacity(reads: usize) -> Self {
        Self {
            reads: Vec::with_capacity(reads),
            write: None,
        }
    }

    /// Returns whether no clipboard work is queued.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.reads.is_empty() && self.write.is_none()
    }

    /// Merges the current [`Clipboard`] requests with others.
    pub fn merge(&mut self, other: &mut Self) {
        self.reads.append(&mut other.reads);

        if let Some(write) = other.write.take() {
            self.write = Some(write);
        }
    }
}

impl Default for Clipboard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Clipboard, Content, Kind};

    #[test]
    fn merge_appends_reads_and_only_replaces_write_when_supplied() {
        let mut current = Clipboard {
            reads: vec![Kind::Text],
            write: Some(Content::Text("old".into())),
        };
        let mut reads_only = Clipboard {
            reads: vec![Kind::Html],
            write: None,
        };

        current.merge(&mut reads_only);
        assert_eq!(current.reads, [Kind::Text, Kind::Html]);
        assert_eq!(current.write, Some(Content::Text("old".into())));
        assert!(reads_only.is_empty());

        let mut replacement = Clipboard {
            reads: Vec::new(),
            write: Some(Content::Text("new".into())),
        };
        current.merge(&mut replacement);
        assert_eq!(current.write, Some(Content::Text("new".into())));
    }

    #[test]
    fn capacity_constructor_starts_empty() {
        assert!(Clipboard::with_read_capacity(8).is_empty());
    }
}
