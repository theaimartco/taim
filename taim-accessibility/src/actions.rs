//! Routing platform actions back to stable widget identity.
//!
//! A screen reader, a switch device, or a voice command names a node and
//! asks for something to happen to it. That request arrives with an
//! [`Id`] captured whenever the platform last read the tree -- which may
//! be several frames ago, across a rebuild, or after the node stopped
//! accepting the action entirely.
//!
//! [`Router`] is the check between those two facts. `phase.md`'s
//! `Actions` vocabulary already states the contract it enforces:
//!
//! > An action being present in this set is a claim that invoking it is
//! > currently valid; a disabled control should have an empty `Actions`
//! > set rather than reporting actions it will reject.
//!
//! So routing is not "find the node and call it": it is "does this node
//! still exist, and does it still claim this action *right now*".

use crate::tree::Tree;

use taim_core::semantics::Actions;
use taim_core::widget::Id;

/// A request to perform one action on one node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dispatch {
    /// The node the platform is asking about.
    pub target: Id,
    /// The single action requested.
    ///
    /// Exactly one flag: a platform asks for one thing at a time, and a
    /// request carrying several would have no meaningful failure mode
    /// when only some of them are valid.
    pub action: Actions,
}

/// Why a dispatch was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    /// The target no longer exists. Expected and harmless: the platform's
    /// view of the tree is always slightly behind.
    Unknown,
    /// The target exists but does not currently claim this action (it was
    /// disabled, or its state changed since the platform last looked).
    NotOffered,
    /// The request named zero actions, or more than one.
    Malformed,
}

/// Validates and routes incoming platform actions.
///
/// Stateless: the tree is the only authority on what is currently valid,
/// so there is nothing to keep in sync.
#[derive(Debug, Clone, Copy, Default)]
pub struct Router;

impl Router {
    /// Creates a router.
    pub fn new() -> Self {
        Self
    }

    /// Validates `dispatch` against `tree`, returning the target's
    /// identity if the action is genuinely available right now.
    ///
    /// The caller performs the action; this decides whether it may. That
    /// split keeps the security-relevant check in one place instead of
    /// spread across every widget's action handler.
    pub fn route(&self, tree: &Tree, dispatch: &Dispatch) -> Result<Id, Rejection> {
        if dispatch.action.iter().count() != 1 {
            return Err(Rejection::Malformed);
        }

        let Some(entry) = tree.get(&dispatch.target) else {
            return Err(Rejection::Unknown);
        };

        if !entry.actions.contains(dispatch.action) {
            return Err(Rejection::NotOffered);
        }

        Ok(entry.id.clone())
    }

    /// Whether `tree` currently offers `action` on `target`.
    pub fn offers(&self, tree: &Tree, target: &Id, action: Actions) -> bool {
        tree.get(target)
            .is_some_and(|entry| entry.actions.contains(action))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use taim_core::semantics::{Node, Role, State};
    use taim_core::{Rectangle, Size};

    fn button(id: &Id, actions: Actions) -> Node {
        Node::new(
            id.clone(),
            Rectangle::with_size(Size::new(10.0, 10.0)),
            Role::Button,
        )
        .with_actions(actions)
    }

    #[test]
    fn an_offered_action_routes_to_its_node() {
        let id = Id::unique();
        let tree = Tree::from_forest(&[button(&id, Actions::ACTIVATE | Actions::FOCUS)]);

        let routed = Router::new().route(
            &tree,
            &Dispatch {
                target: id.clone(),
                action: Actions::ACTIVATE,
            },
        );

        assert_eq!(routed, Ok(id));
    }

    #[test]
    fn an_action_the_node_never_offered_is_refused() {
        let id = Id::unique();
        let tree = Tree::from_forest(&[button(&id, Actions::FOCUS)]);

        assert_eq!(
            Router::new().route(
                &tree,
                &Dispatch {
                    target: id,
                    action: Actions::ACTIVATE,
                },
            ),
            Err(Rejection::NotOffered)
        );
    }

    #[test]
    fn a_stale_target_is_refused_rather_than_panicking() {
        // The platform's view always lags: a request for a node that has
        // since vanished is routine, not exceptional.
        let gone = Id::unique();
        let tree = Tree::from_forest(&[]);

        assert_eq!(
            Router::new().route(
                &tree,
                &Dispatch {
                    target: gone,
                    action: Actions::ACTIVATE,
                },
            ),
            Err(Rejection::Unknown)
        );
    }

    #[test]
    fn an_action_on_a_now_disabled_control_is_refused() {
        // The realistic race: the platform read the tree while the button
        // was live, the user disabled it, and the request arrives after.
        let id = Id::unique();

        let live = Tree::from_forest(&[button(&id, Actions::ACTIVATE | Actions::FOCUS)]);
        let disabled =
            Tree::from_forest(&[button(&id, Actions::empty()).with_state(State::DISABLED)]);

        let router = Router::new();
        let dispatch = Dispatch {
            target: id.clone(),
            action: Actions::ACTIVATE,
        };

        assert_eq!(router.route(&live, &dispatch), Ok(id));
        assert_eq!(
            router.route(&disabled, &dispatch),
            Err(Rejection::NotOffered),
            "a control that stopped offering an action must reject it"
        );
    }

    #[test]
    fn a_malformed_request_naming_zero_or_many_actions_is_refused() {
        let id = Id::unique();
        let tree = Tree::from_forest(&[button(&id, Actions::all())]);
        let router = Router::new();

        assert_eq!(
            router.route(
                &tree,
                &Dispatch {
                    target: id.clone(),
                    action: Actions::empty(),
                },
            ),
            Err(Rejection::Malformed)
        );

        assert_eq!(
            router.route(
                &tree,
                &Dispatch {
                    target: id,
                    action: Actions::ACTIVATE | Actions::FOCUS,
                },
            ),
            Err(Rejection::Malformed),
            "a request must name exactly one action, or partial failure is undefined"
        );
    }

    #[test]
    fn offers_agrees_with_route() {
        let id = Id::unique();
        let tree = Tree::from_forest(&[button(&id, Actions::ACTIVATE)]);
        let router = Router::new();

        assert!(router.offers(&tree, &id, Actions::ACTIVATE));
        assert!(!router.offers(&tree, &id, Actions::DISMISS));
        assert!(!router.offers(&tree, &Id::unique(), Actions::ACTIVATE));
    }
}
