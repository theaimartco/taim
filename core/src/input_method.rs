//! Listen to input method events.
mod event;
mod model;
mod security;

pub use event::Event;
pub use model::{InputMethod, Preedit, Purpose};
pub use security::{CandidateRect, SurroundingText};
