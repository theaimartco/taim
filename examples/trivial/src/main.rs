//! Minimal TAIM window, also used by the external consumer smoke check.
use taim::prelude::*;

pub fn main() -> taim::Result {
    taim::launch("Taim — Trivial", |_| text("Hello, Taim!").size(24))
}
