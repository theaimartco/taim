//! Top-level harness for the GPU hot-path test suite.
//!
//! Registers every module under `tests/gpu/`.

#[path = "gpu/batching.rs"]
mod batching;

#[path = "gpu/device_loss.rs"]
mod device_loss;

#[path = "gpu/cache_pressure.rs"]
mod cache_pressure;

#[path = "gpu/effect_bounds.rs"]
mod effect_bounds;

#[path = "gpu/effect_budget.rs"]
mod effect_budget;
