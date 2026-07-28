//! Top-level harness for the smoke test suite.
//!
//! Registers every module under `tests/smoke/`. New smoke categories are
//! added here as `mod` declarations; the phase that adds the first file in
//! a new test category is responsible for creating its own top-level
//! harness (see phase.md section 2).

#[path = "smoke/workspace.rs"]
mod workspace;
