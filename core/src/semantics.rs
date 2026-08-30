//! Platform-neutral semantic tree types.
//!
//! [`Node`] describes what a widget subtree means to an assistive
//! technology (a screen reader, a switch-style navigator) without
//! committing to any specific platform accessibility API. Phase 13's
//! `taim-accessibility` crate builds the actual incremental tree,
//! platform adapters, and focus/live-region behavior on top of this
//! vocabulary; Phase 2 only defines the shape widgets will eventually
//! report.
//!
//! No [`Widget`] method is mandatory yet -- [`crate::Widget::semantics`]
//! is a defaulted method that reports [`None`], so this module can be
//! adopted incrementally, widget by widget, starting in Phase 13.
//!
//! [`Widget`]: crate::Widget

use crate::widget::Id;
use crate::{Rectangle, SmolStr};

/// A single node in the semantic tree.
///
/// A [`Node`] is a value, not a live handle: it is produced fresh (or
/// reused, once Phase 13 adds dirty-subtree tracking) each time a
/// widget's semantics are requested, and consumed by a platform adapter
/// or test oracle.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// The stable identity of the widget this node describes.
    ///
    /// Required (not optional) because every downstream consumer --
    /// action routing, focus restoration, incremental diffing -- needs a
    /// stable key, and a semantic node with no identity cannot be acted
    /// upon or found again after a rebuild.
    pub id: Id,
    /// The screen-space bounds of the widget this node describes.
    pub bounds: Rectangle,
    /// What kind of element this is.
    pub role: Role,
    /// The current [`State`] flags of the widget.
    pub state: State,
    /// A short, human-readable name (e.g. a button's label). Required by
    /// most assistive technologies to announce the element at all.
    pub name: Option<SmolStr>,
    /// A longer description supplementing [`Node::name`] (e.g. a
    /// tooltip), read on request rather than automatically.
    pub description: Option<SmolStr>,
    /// The current value of the widget, if it holds one (e.g. a slider's
    /// position or a text field's content).
    ///
    /// Private, and reachable only through [`Node::with_value`] and
    /// [`Node::value`], because both of those *enforce* the rule that a
    /// [`Role::SecureField`] never discloses a value. A public field
    /// would make that rule advisory: any caller could assign a password
    /// straight into it, and the security requirement in `phase.md`
    /// Phase 10 would be a comment rather than a guarantee.
    value: Option<Value>,
    /// The actions this node currently supports.
    pub actions: Actions,
    /// The children of this node, in traversal (typically visual) order.
    pub children: Vec<Node>,
    /// The live region behavior of this node, if it announces changes
    /// proactively (e.g. a toast or a validation message).
    pub live: Option<Live>,
}

impl Node {
    /// Creates a new, childless [`Node`] with the given identity, bounds,
    /// and [`Role`], and no other properties set.
    pub fn new(id: Id, bounds: Rectangle, role: Role) -> Self {
        Self {
            id,
            bounds,
            role,
            state: State::empty(),
            name: None,
            description: None,
            value: None,
            actions: Actions::empty(),
            children: Vec::new(),
            live: None,
        }
    }

    /// Sets the [`Node::name`].
    pub fn with_name(mut self, name: impl Into<SmolStr>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the [`Node::description`].
    pub fn with_description(mut self, description: impl Into<SmolStr>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the [`Node::state`].
    pub fn with_state(mut self, state: State) -> Self {
        self.state = state;
        self
    }

    /// Sets the [`Node::live`] region behavior.
    ///
    /// A live region announces changes the user did not cause and is not
    /// looking at. Whether an announcement is actually *made* is not
    /// decided here: `taim_accessibility::LiveRegions::announce` enforces
    /// the privacy rules (a [`Role::SecureField`] never announces, no
    /// matter what this is set to).
    pub fn with_live(mut self, live: Live) -> Self {
        self.live = Some(live);
        self
    }

    /// Sets this node's value.
    ///
    /// # Security
    ///
    /// A [`Role::SecureField`] node **ignores** this call and keeps no
    /// value: a password must never reach an assistive-technology bridge,
    /// which is one of the disclosure channels `phase.md` Phase 10's
    /// security requirements name explicitly.
    ///
    /// This fails safe -- silently withholding -- rather than panicking.
    /// A crash on the exact code path that handles a password is itself a
    /// hazard (it turns a caller's mistake into a denial of service, and
    /// panic messages are their own disclosure channel), and there is no
    /// legitimate reason to give a secure field a value, so there is
    /// nothing for a louder failure to save. Refusal *is* the guarantee:
    /// [`Node::value`] is private, so this is the only way to set one.
    pub fn with_value(mut self, value: Value) -> Self {
        // Asked of the classification, not of the role directly: the rule
        // is "a secure field discloses nothing to the accessibility tree",
        // and `security` is where that rule lives. Re-deciding it here
        // with a `==` is how the fourth copy of a rule ends up disagreeing
        // with the other three.
        if !self
            .role
            .privacy()
            .allows(crate::security::Audience::Semantics)
        {
            return self;
        }

        self.value = Some(value);
        self
    }

    /// Returns this node's value, if it discloses one.
    ///
    /// Always `None` for a [`Role::SecureField`], regardless of how the
    /// node was built -- the read path enforces the same rule as
    /// [`Node::with_value`], so a secure field cannot disclose a value
    /// even if one were somehow set.
    pub fn value(&self) -> Option<&Value> {
        if !self
            .role
            .privacy()
            .allows(crate::security::Audience::Semantics)
        {
            return None;
        }

        self.value.as_ref()
    }

    /// Sets the [`Node::actions`].
    pub fn with_actions(mut self, actions: Actions) -> Self {
        self.actions = actions;
        self
    }

    /// Appends a child to [`Node::children`].
    pub fn with_child(mut self, child: Node) -> Self {
        self.children.push(child);
        self
    }
}

/// What kind of element a [`Node`] represents.
///
/// This list mirrors the common subset of platform accessibility roles
/// (ARIA, `AXRole`, `UIA` control types) that Taim's own widgets need;
/// it is deliberately not exhaustive of every platform's role vocabulary,
/// since platform adapters (Phase 13) are responsible for mapping an
/// Taim [`Role`] onto whatever the local platform expects, including
/// roles this enum has no equivalent for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Role {
    /// Content with no interactive behavior (e.g. a paragraph of text).
    StaticText,
    /// An image or other graphic with a textual alternative.
    Image,
    /// A clickable control that performs an action.
    Button,
    /// A two-state toggle (on/off).
    Checkbox,
    /// A single choice within a mutually exclusive group.
    RadioButton,
    /// A continuous or stepped value picker.
    Slider,
    /// A single-line or multi-line editable text field.
    TextField,
    /// A text field whose [`Node::value`] must never be populated; see
    /// the security requirements in `phase.md` Phase 10.
    SecureField,
    /// A scrollable region.
    ScrollView,
    /// A container with no semantic meaning of its own, grouping
    /// children for layout purposes only.
    Group,
    /// A named grouping of related controls (e.g. a form section).
    NamedGroup,
    /// A link that navigates elsewhere.
    Link,
    /// A tab within a tab list.
    Tab,
    /// A menu item.
    MenuItem,
    /// A row within a list or table.
    ListItem,
    /// The root of a window or dialog.
    Window,
}

impl Role {
    /// How sensitive a node of this role is, by construction.
    ///
    /// [`Role::SecureField`] is [`Privacy::Secret`]: the role *is* the
    /// classification. A password field is not secret because someone
    /// remembered to mark it — it is secret because of what it is, so the
    /// accessibility tree, the profiler, and every platform adapter can
    /// work that out for themselves rather than trusting a separate flag
    /// that can be forgotten.
    ///
    /// Every other role is [`Privacy::Public`] *as a role*. A node may
    /// still be more restrictive than its role — a notification body is
    /// [`Role::StaticText`] and [`Privacy::Private`] — so this is a
    /// **floor**, not the answer. A caller that knows more takes
    /// [`Privacy::max`] of the two.
    ///
    /// [`Privacy`]: crate::security::Privacy
    /// [`Privacy::Secret`]: crate::security::Privacy::Secret
    /// [`Privacy::Public`]: crate::security::Privacy::Public
    /// [`Privacy::Private`]: crate::security::Privacy::Private
    /// [`Privacy::max`]: crate::security::Privacy::max
    pub fn privacy(self) -> crate::security::Privacy {
        use crate::security::Privacy;

        match self {
            Role::SecureField => Privacy::Secret,
            _ => Privacy::Public,
        }
    }
}

bitflags::bitflags! {
    /// The current state flags of a [`Node`].
    ///
    /// Modeled as flags (rather than separate booleans) so a platform
    /// adapter can forward the whole set in one call, and so tests can
    /// assert on a specific combination directly.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct State: u16 {
        /// The node is disabled and cannot be interacted with.
        const DISABLED = 1 << 0;
        /// The node is selected (e.g. a chosen tab or list item).
        const SELECTED = 1 << 1;
        /// The node currently has keyboard focus.
        const FOCUSED = 1 << 2;
        /// The node can receive keyboard focus (even if it does not
        /// currently have it).
        const FOCUSABLE = 1 << 3;
        /// The node is checked (for [`Role::Checkbox`]/[`Role::RadioButton`]).
        const CHECKED = 1 << 4;
        /// The node is in an indeterminate/mixed state (e.g. a checkbox
        /// representing a partial selection).
        const INDETERMINATE = 1 << 5;
        /// The node is expanded (e.g. an open menu or disclosure).
        const EXPANDED = 1 << 6;
        /// The node is hidden from the semantic tree but still present
        /// in the widget tree (e.g. mid-transition).
        const HIDDEN = 1 << 7;
        /// The node is busy loading and its content should not be
        /// announced yet.
        const BUSY = 1 << 8;
    }
}

/// The current value of a [`Node`], typed by what kind of value it is.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A textual value (e.g. a text field's content).
    ///
    /// Never populate this for [`Role::SecureField`].
    Text(SmolStr),
    /// A numeric value with its valid range (e.g. a slider).
    Number {
        /// The current value.
        current: f64,
        /// The minimum valid value.
        min: f64,
        /// The maximum valid value.
        max: f64,
    },
}

bitflags::bitflags! {
    /// The actions a [`Node`] currently supports.
    ///
    /// An action being present in this set is a claim that invoking it
    /// (via [`Action`]) is currently valid; a disabled control should
    /// have an empty [`Actions`] set rather than reporting actions it
    /// will reject.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct Actions: u16 {
        /// The node can be activated (pressed, clicked, or invoked via
        /// keyboard).
        const ACTIVATE = 1 << 0;
        /// The node can receive focus.
        const FOCUS = 1 << 1;
        /// The node's value can be increased in discrete steps.
        const INCREMENT = 1 << 2;
        /// The node's value can be decreased in discrete steps.
        const DECREMENT = 1 << 3;
        /// The node's value can be set directly (e.g. a slider drag).
        const SET_VALUE = 1 << 4;
        /// The node can be expanded or collapsed.
        const TOGGLE_EXPANDED = 1 << 5;
        /// The node can be scrolled into view.
        const SCROLL_INTO_VIEW = 1 << 6;
        /// The node can be dismissed (e.g. a toast or a modal).
        const DISMISS = 1 << 7;
    }
}

/// A request to perform an [`Actions`] member against a specific node,
/// routed by its stable [`Id`].
///
/// Phase 13's `taim-accessibility::actions` module is the eventual owner
/// of turning a platform action invocation into an [`Action`] and
/// routing it back to the originating widget; Phase 2 only defines its
/// shape.
#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    /// The identity of the target node, matching [`Node::id`].
    pub target: Id,
    /// The action being requested.
    pub request: Request,
}

/// The specific action being requested of a node.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// Activate the node (see [`Actions::ACTIVATE`]).
    Activate,
    /// Focus the node (see [`Actions::FOCUS`]).
    Focus,
    /// Increment the node's value (see [`Actions::INCREMENT`]).
    Increment,
    /// Decrement the node's value (see [`Actions::DECREMENT`]).
    Decrement,
    /// Set the node's value directly (see [`Actions::SET_VALUE`]).
    SetValue(Value),
    /// Toggle the node's expanded state (see [`Actions::TOGGLE_EXPANDED`]).
    ToggleExpanded,
    /// Scroll the node into view (see [`Actions::SCROLL_INTO_VIEW`]).
    ScrollIntoView,
    /// Dismiss the node (see [`Actions::DISMISS`]).
    Dismiss,
}

/// How a [`Node`] announces changes to its content proactively, without
/// the assistive technology needing to be focused on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Live {
    /// Announce changes without interrupting the current speech (e.g. a
    /// status message).
    Polite,
    /// Announce changes immediately, interrupting current speech (e.g.
    /// an error that demands attention).
    Assertive,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Point, Size};

    fn bounds() -> Rectangle {
        Rectangle::new(Point::ORIGIN, Size::new(100.0, 40.0))
    }

    #[test]
    fn new_node_has_no_state_or_actions_by_default() {
        let node = Node::new(Id::unique(), bounds(), Role::Button);

        assert_eq!(node.state, State::empty());
        assert_eq!(node.actions, Actions::empty());
        assert!(node.children.is_empty());
    }

    #[test]
    fn builder_methods_set_the_expected_fields() {
        let node = Node::new(Id::unique(), bounds(), Role::Button)
            .with_name("Submit")
            .with_state(State::FOCUSABLE | State::FOCUSED)
            .with_actions(Actions::ACTIVATE | Actions::FOCUS);

        assert_eq!(node.name.as_deref(), Some("Submit"));
        assert!(node.state.contains(State::FOCUSABLE));
        assert!(node.state.contains(State::FOCUSED));
        assert!(node.actions.contains(Actions::ACTIVATE));
    }

    #[test]
    fn secure_field_role_exists_and_carries_no_special_value_enforcement_here() {
        // The type system cannot forbid populating `value` for a
        // `SecureField` node (that enforcement is Phase 10/13's
        // responsibility, at the point where widgets actually produce
        // `Node`s); this test only pins the role's existence so a
        // future violation of the documented contract is easy to grep
        // for via `Role::SecureField`.
        let node = Node::new(Id::unique(), bounds(), Role::SecureField);

        assert_eq!(node.role, Role::SecureField);
        assert_eq!(node.value, None);
    }

    #[test]
    fn with_child_appends_in_order() {
        let child_a = Node::new(Id::unique(), bounds(), Role::StaticText);
        let child_b = Node::new(Id::unique(), bounds(), Role::StaticText);

        let parent = Node::new(Id::unique(), bounds(), Role::Group)
            .with_child(child_a.clone())
            .with_child(child_b.clone());

        assert_eq!(parent.children, vec![child_a, child_b]);
    }

    #[test]
    fn action_targets_a_node_by_stable_id() {
        let id = Id::keyed(7_u64);

        let action = Action {
            target: id.clone(),
            request: Request::Activate,
        };

        assert_eq!(action.target, id);
    }
}
