//! Use the built-in theme and styles.
pub mod palette;

pub use palette::Palette;

mod builtin;
mod semantic;

pub use builtin::*;
pub use semantic::*;
