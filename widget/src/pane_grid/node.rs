use crate::pane_grid::{Axis, Pane, Split};

#[path = "node/navigation.rs"]
mod navigation;
#[path = "node/regions.rs"]
mod regions;

/// A layout node of a [`PaneGrid`].
///
/// [`PaneGrid`]: super::PaneGrid
#[derive(Debug, Clone)]
pub enum Node {
    /// The region of this [`Node`] is split into two.
    Split {
        /// The [`Split`] of this [`Node`].
        id: Split,

        /// The direction of the split.
        axis: Axis,

        /// The ratio of the split in [0.0, 1.0].
        ratio: f32,

        /// The left/top [`Node`] of the split.
        a: Box<Node>,

        /// The right/bottom [`Node`] of the split.
        b: Box<Node>,
    },
    /// The region of this [`Node`] is taken by a [`Pane`].
    Pane(Pane),
}

impl Node {
    /// Returns an iterator over each [`Split`] in this [`Node`].
    pub fn splits(&self) -> impl Iterator<Item = &Split> {
        let mut unvisited_nodes = vec![self];

        std::iter::from_fn(move || {
            while let Some(node) = unvisited_nodes.pop() {
                if let Node::Split { id, a, b, .. } = node {
                    unvisited_nodes.push(a);
                    unvisited_nodes.push(b);

                    return Some(id);
                }
            }

            None
        })
    }

    pub(crate) fn find(&mut self, pane: Pane) -> Option<&mut Node> {
        match self {
            Node::Split { a, b, .. } => a.find(pane).or_else(move || b.find(pane)),
            Node::Pane(p) => {
                if *p == pane {
                    Some(self)
                } else {
                    None
                }
            }
        }
    }

    pub(crate) fn split(&mut self, id: Split, axis: Axis, new_pane: Pane) {
        let previous = std::mem::replace(self, Node::Pane(new_pane));

        *self = Node::Split {
            id,
            axis,
            ratio: 0.5,
            a: Box::new(previous),
            b: Box::new(Node::Pane(new_pane)),
        };
    }

    pub(crate) fn split_inverse(&mut self, id: Split, axis: Axis, pane: Pane) {
        let previous = std::mem::replace(self, Node::Pane(pane));

        *self = Node::Split {
            id,
            axis,
            ratio: 0.5,
            a: Box::new(Node::Pane(pane)),
            b: Box::new(previous),
        };
    }

    pub(crate) fn swap_panes(&mut self, a: Pane, b: Pane) -> u8 {
        match self {
            Node::Pane(pane) if *pane == a => {
                *pane = b;
                0b01
            }
            Node::Pane(pane) if *pane == b => {
                *pane = a;
                0b10
            }
            Node::Pane(_) => 0,
            Node::Split {
                a: left, b: right, ..
            } => {
                let found = left.swap_panes(a, b);

                if found == 0b11 {
                    found
                } else {
                    found | right.swap_panes(a, b)
                }
            }
        }
    }

    pub(crate) fn relabel(&mut self, target: Pane, label: Pane) -> bool {
        match self {
            Node::Pane(pane) if *pane == target => {
                *pane = label;
                true
            }
            Node::Pane(_) => false,
            Node::Split { a, b, .. } => a.relabel(target, label) || b.relabel(target, label),
        }
    }

    pub(crate) fn resize(&mut self, split: Split, percentage: f32) -> bool {
        match self {
            Node::Split {
                id, ratio, a, b, ..
            } => {
                if *id == split {
                    *ratio = percentage;

                    true
                } else if a.resize(split, percentage) {
                    true
                } else {
                    b.resize(split, percentage)
                }
            }
            Node::Pane(_) => false,
        }
    }

    pub(crate) fn remove(&mut self, pane: Pane) -> Option<Pane> {
        match self {
            Node::Split { a, b, .. } => {
                if a.pane() == Some(pane) {
                    let sibling = b.first_pane();
                    let replacement = std::mem::replace(&mut **b, Node::Pane(sibling));
                    *self = replacement;
                    Some(sibling)
                } else if b.pane() == Some(pane) {
                    let sibling = a.first_pane();
                    let replacement = std::mem::replace(&mut **a, Node::Pane(sibling));
                    *self = replacement;
                    Some(sibling)
                } else {
                    a.remove(pane).or_else(|| b.remove(pane))
                }
            }
            Node::Pane(_) => None,
        }
    }

    fn pane(&self) -> Option<Pane> {
        match self {
            Node::Split { .. } => None,
            Node::Pane(pane) => Some(*pane),
        }
    }

    fn first_pane(&self) -> Pane {
        match self {
            Node::Split { a, .. } => a.first_pane(),
            Node::Pane(pane) => *pane,
        }
    }
}

impl std::hash::Hash for Node {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Node::Split {
                id,
                axis,
                ratio,
                a,
                b,
            } => {
                id.hash(state);
                axis.hash(state);
                finite_ratio(*ratio).to_bits().hash(state);
                a.hash(state);
                b.hash(state);
            }
            Node::Pane(pane) => {
                pane.hash(state);
            }
        }
    }
}

#[inline]
fn finite_ratio(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.5
    }
}
