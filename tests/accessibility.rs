//! Top-level harness for the accessibility test suite.
//!
//! Registers every module under `tests/accessibility/`.

#[path = "accessibility/components.rs"]
mod components;

#[path = "accessibility/focus.rs"]
mod focus;

#[path = "accessibility/tree.rs"]
mod tree;

#[path = "accessibility/overlay.rs"]
mod overlay;

#[path = "accessibility/user_interface.rs"]
mod user_interface;

#[path = "accessibility/policies.rs"]
mod policies;

#[path = "accessibility/widgets.rs"]
mod widgets;
