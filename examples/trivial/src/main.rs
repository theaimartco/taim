// No console window behind the app in Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
// The smallest possible real Taim application: a window showing one piece
// of static content. Exists to measure Pillar XIII's "trivial app under
// 1.5 MiB stripped" gate (future.md) -- see `examples/mobile_shell` for a
// realistic app and this crate's `Cargo.toml` for why the feature list
// matches it.
use taim::prelude::*;

pub fn main() -> taim::Result {
    taim::launch("Taim — Trivial", |_cx| text("Hello, Taim!").size(24.0))
}
