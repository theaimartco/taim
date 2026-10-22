//! Thin binary entry point; the showcase lives in `src/lib.rs` so
//! `tests/headless_semantics.rs` can drive the same tree without a window.

pub fn main() -> taim::Result {
    taim_showcase::main()
}
