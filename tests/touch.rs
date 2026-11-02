//! Top-level harness for the touch input test suite.
//!
//! Registers every module under `tests/touch/`.

#[path = "touch/scroll.rs"]
mod scroll;

#[path = "touch/paint.rs"]
mod paint;

#[path = "touch/pinch.rs"]
mod pinch;
