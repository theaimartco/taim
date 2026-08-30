//! Drag-and-drop offer lifecycle.

use std::collections::BTreeSet;

use crate::clipboard::Privacy;

/// Data-transfer action negotiated by source and target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    /// Copy data.
    Copy,
    /// Move data.
    Move,
    /// Ask the user.
    Ask,
}

/// Terminal-safe offer lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Advertised but outside a target.
    Offered,
    /// Inside an accepting target.
    Entered,
    /// Payload was dropped.
    Dropped,
    /// Transfer was cancelled.
    Cancelled,
    /// Transfer completed.
    Finished,
}

/// Validated drag-and-drop offer.
#[derive(Debug)]
pub struct Offer {
    actions: BTreeSet<Action>,
    selected: Option<Action>,
    privacy: Privacy,
    state: State,
}

impl Offer {
    /// Creates an offer with supported actions and privacy.
    pub fn new(actions: impl IntoIterator<Item = Action>, privacy: Privacy) -> Self {
        Self {
            actions: actions.into_iter().collect(),
            selected: None,
            privacy,
            state: State::Offered,
        }
    }

    /// Enters a target if disclosure is permitted.
    pub fn enter(&mut self) -> bool {
        if self.state != State::Offered || self.privacy == Privacy::Secret {
            return false;
        }
        self.state = State::Entered;
        true
    }

    /// Selects a mutually supported action.
    pub fn select(&mut self, action: Action) -> bool {
        if self.state != State::Entered || !self.actions.contains(&action) {
            return false;
        }
        self.selected = Some(action);
        true
    }

    /// Records a drop after action negotiation.
    pub fn drop_data(&mut self) -> bool {
        if self.state != State::Entered || self.selected.is_none() {
            return false;
        }
        self.state = State::Dropped;
        true
    }

    /// Completes a dropped transfer.
    pub fn finish(&mut self) -> bool {
        if self.state != State::Dropped {
            return false;
        }
        self.state = State::Finished;
        true
    }

    /// Cancels every non-finished transfer.
    pub fn cancel(&mut self) {
        if self.state != State::Finished {
            self.state = State::Cancelled;
        }
    }

    /// Returns the current lifecycle state.
    pub const fn state(&self) -> State {
        self.state
    }
}
