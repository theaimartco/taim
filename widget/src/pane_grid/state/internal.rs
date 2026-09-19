use crate::pane_grid::{Configuration, Node, Pane, Split};

use std::borrow::Cow;
use std::collections::BTreeMap;

/// Internal pane-tree state and identifier allocator.
#[derive(Debug, Clone)]
pub struct Internal {
    pub(super) layout: Node,
    pub(super) last_id: usize,
    pub(super) maximized: Option<Pane>,
}

impl Internal {
    /// Builds an internal tree and pane map from a configuration.
    pub fn from_configuration<T>(
        panes: &mut BTreeMap<Pane, T>,
        content: Configuration<T>,
        next_id: usize,
    ) -> Self {
        let (layout, last_id) = match content {
            Configuration::Split { axis, ratio, a, b } => {
                let left = Self::from_configuration(panes, *a, next_id);
                let right = Self::from_configuration(panes, *b, left.last_id);
                let split_id = right.last_id;

                (
                    Node::Split {
                        id: Split(split_id),
                        axis,
                        ratio,
                        a: Box::new(left.layout),
                        b: Box::new(right.layout),
                    },
                    split_id
                        .checked_add(1)
                        .expect("pane-grid identifier space exhausted"),
                )
            }
            Configuration::Pane(state) => {
                let id = Pane(next_id);
                let _ = panes.insert(id, state);

                (
                    Node::Pane(id),
                    next_id
                        .checked_add(1)
                        .expect("pane-grid identifier space exhausted"),
                )
            }
        };

        Self {
            layout,
            last_id,
            maximized: None,
        }
    }

    pub(in crate::pane_grid) fn layout(&self) -> Cow<'_, Node> {
        self.maximized.map_or_else(
            || Cow::Borrowed(&self.layout),
            |pane| Cow::Owned(Node::Pane(pane)),
        )
    }

    pub(in crate::pane_grid) fn maximized(&self) -> Option<Pane> {
        self.maximized
    }
}
