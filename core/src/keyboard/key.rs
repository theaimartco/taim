//! Logical and physical keyboard identity.
//!
//! TAIM keeps logical meaning separate from physical position. The catalogs
//! follow the platform-neutral UI Events key/code vocabulary, while native
//! codes remain available for unmapped hardware.

mod logical;
mod physical;

pub use logical::{Key, Named};
pub use physical::{Code, NativeCode, Physical};
