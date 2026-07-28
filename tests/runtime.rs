//! Top-level harness for the runtime scheduling test suite.
//!
//! Registers every module under `tests/runtime/`.

#[path = "runtime/frame_scheduler.rs"]
mod frame_scheduler;

#[path = "runtime/idle.rs"]
mod idle;

#[path = "runtime/occlusion.rs"]
mod occlusion;
