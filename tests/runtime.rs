//! Top-level harness for the runtime scheduling test suite.
//!
//! Registers every module under `tests/runtime/`.

#[path = "runtime/frame_scheduler.rs"]
mod frame_scheduler;

#[path = "runtime/idle.rs"]
mod idle;

#[path = "runtime/occlusion.rs"]
mod occlusion;

#[path = "runtime/retained_replay_probe.rs"]
mod retained_replay_probe;

#[path = "runtime/phase3_inversion.rs"]
mod phase3_inversion;
