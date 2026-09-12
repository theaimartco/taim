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

#[path = "gpu/display_transform.rs"]
mod display_transform;

#[path = "gpu/target_formats.rs"]
mod target_formats;

#[path = "gpu/quality_budget.rs"]
mod quality_budget;

#[path = "gpu/color_output.rs"]
mod color_output;

#[path = "gpu/material_shading.rs"]
mod material_shading;

#[path = "gpu/material_live.rs"]
mod material_live;

#[path = "gpu/material_backdrop.rs"]
mod material_backdrop;

#[path = "gpu/compute_rasterizer.rs"]
mod compute_rasterizer;
