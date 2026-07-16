//! Top-level harness for the widget-level test suite.
//!
//! Registers every module under `tests/widgets/`.

#[path = "widgets/identity.rs"]
mod identity;

#[path = "widgets/dirty_flags.rs"]
mod dirty_flags;

#[path = "widgets/invalidation.rs"]
mod invalidation;
