//! Top-level harness for the layout test suite.
//!
//! Registers every module under `tests/layout/`.

#[path = "layout/incremental.rs"]
mod incremental;

#[path = "layout/property.rs"]
mod property;
