//! Top-level harness for the application-building suite: navigation,
//! long lists and platform services, the way an application uses them.
//!
//! Registers every module under `tests/apps/`.

#[path = "apps/support.rs"]
mod support;

#[path = "apps/navigator.rs"]
mod navigator;

#[path = "apps/list.rs"]
mod list;
