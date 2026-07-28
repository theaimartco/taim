//! Top-level harness for the profiler test suite.
//!
//! Registers every module under `tests/profiler/`.

#[path = "profiler/overhead.rs"]
mod overhead;

#[path = "profiler/pipeline.rs"]
mod pipeline;

#[path = "profiler/privacy.rs"]
mod privacy;
