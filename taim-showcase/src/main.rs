//! Thin binary entry point.
//!
//! The showcase itself lives in `src/lib.rs`, generic over its renderer,
//! so `tests/headless_semantics.rs` can drive the identical widget tree
//! with the null renderer instead of the real one this binary uses.

pub fn main() -> taim::Result {
    taim_showcase::main()
}
