//! Top-level harness for the gesture-arena test suite.
//!
//! Registers every module under `tests/gestures/`.

#[path = "gestures/arena.rs"]
mod arena;

#[path = "gestures/nested_scroll.rs"]
mod nested_scroll;

#[path = "gestures/recorded_streams.rs"]
mod recorded_streams;
