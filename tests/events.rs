//! Top-level harness for the event-path test suite.
//!
//! Registers every module under `tests/events/`.

#[path = "events/hit_path.rs"]
mod hit_path;

#[path = "events/spatial_hit_test.rs"]
mod spatial_hit_test;

#[path = "events/user_interface.rs"]
mod user_interface;
