//! Top-level harness for the colour-pipeline contract suite.
//!
//! Registers every module under `tests/color/`.

#[path = "color/contract.rs"]
mod contract;

#[path = "color/output.rs"]
mod output;
