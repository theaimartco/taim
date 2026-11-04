//! Listen to input method events.
mod editor_state;
mod event;
pub use editor_state::{EditorState, utf8_to_utf16, utf16_to_utf8};
mod model;
mod security;

pub use event::Event;
pub use model::{InputMethod, Preedit, Purpose};
pub use security::{CandidateRect, SurroundingText};
