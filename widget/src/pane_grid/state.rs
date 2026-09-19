//! The state of a [`PaneGrid`].
//!
//! [`PaneGrid`]: super::PaneGrid
use crate::pane_grid::{Configuration, Node, Pane};

use std::collections::BTreeMap;

mod action;
mod internal;
mod operations;

pub use action::Action;
pub use internal::Internal;

/// The state of a [`PaneGrid`].
///
/// It keeps track of the state of each [`Pane`] and the position of each
/// [`Split`].
///
/// The [`State`] needs to own any mutable contents a [`Pane`] may need. This is
/// why this struct is generic over the type `T`. Values of this type are
/// provided to the view function of [`PaneGrid::new`] for displaying each
/// [`Pane`].
///
/// [`PaneGrid`]: super::PaneGrid
/// [`PaneGrid::new`]: super::PaneGrid::new
#[derive(Debug, Clone)]
pub struct State<T> {
    /// The panes of the [`PaneGrid`].
    ///
    /// [`PaneGrid`]: super::PaneGrid
    pub panes: BTreeMap<Pane, T>,

    /// The internal state of the [`PaneGrid`].
    ///
    /// [`PaneGrid`]: super::PaneGrid
    pub internal: Internal,
}

impl<T> State<T> {
    /// Creates a new [`State`], initializing the first pane with the provided
    /// state.
    ///
    /// Alongside the [`State`], it returns the first [`Pane`] identifier.
    pub fn new(first_pane_state: T) -> (Self, Pane) {
        (
            Self::with_configuration(Configuration::Pane(first_pane_state)),
            Pane(0),
        )
    }

    /// Creates a new [`State`] with the given [`Configuration`].
    pub fn with_configuration(config: impl Into<Configuration<T>>) -> Self {
        let mut panes = BTreeMap::default();

        let internal = Internal::from_configuration(&mut panes, config.into(), 0);

        State { panes, internal }
    }

    /// Returns the total amount of panes in the [`State`].
    pub fn len(&self) -> usize {
        self.panes.len()
    }

    /// Returns `true` if the amount of panes in the [`State`] is 0.
    pub fn is_empty(&self) -> bool {
        self.panes.is_empty()
    }

    /// Returns the internal state of the given [`Pane`], if it exists.
    pub fn get(&self, pane: Pane) -> Option<&T> {
        self.panes.get(&pane)
    }

    /// Returns the internal state of the given [`Pane`] with mutability, if it
    /// exists.
    pub fn get_mut(&mut self, pane: Pane) -> Option<&mut T> {
        self.panes.get_mut(&pane)
    }

    /// Returns an iterator over all the panes of the [`State`], alongside its
    /// internal state.
    pub fn iter(&self) -> impl Iterator<Item = (&Pane, &T)> {
        self.panes.iter()
    }

    /// Returns a mutable iterator over all the panes of the [`State`],
    /// alongside its internal state.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&Pane, &mut T)> {
        self.panes.iter_mut()
    }

    /// Returns the layout of the [`State`].
    pub fn layout(&self) -> &Node {
        &self.internal.layout
    }
}
