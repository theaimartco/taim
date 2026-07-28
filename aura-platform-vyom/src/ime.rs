//! Text-input-v3 session state with secure-field disclosure enforcement.

use std::ops::Range;

use aura_core::Rectangle;
use aura_core::input_method::{Purpose, SurroundingText};

/// Event translated from text-input-v3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// In-progress composition.
    Preedit {
        /// Preedit content.
        text: String,
        /// Optional UTF-8 byte selection.
        selection: Option<Range<usize>>,
    },
    /// Committed text.
    Commit(String),
    /// Request to delete around the cursor.
    DeleteSurrounding {
        /// Bytes before the cursor.
        before: usize,
        /// Bytes after the cursor.
        after: usize,
    },
    /// Atomic protocol batch completed.
    Done(u32),
    /// Session was cancelled.
    Cancelled,
}

/// Active input-method session.
#[derive(Debug, Clone)]
pub struct Session {
    purpose: Purpose,
    surrounding: SurroundingText,
    candidate: Rectangle,
    serial: u32,
    enabled: bool,
}

impl Session {
    /// Starts a session while enforcing secure-field redaction.
    pub fn new(purpose: Purpose, surrounding: SurroundingText, candidate: Rectangle) -> Self {
        let surrounding = if purpose == Purpose::Secure {
            SurroundingText::Redacted
        } else {
            surrounding
        };
        Self {
            purpose,
            surrounding,
            candidate,
            serial: 0,
            enabled: true,
        }
    }

    /// Returns platform-visible surrounding text.
    pub fn surrounding_text(&self) -> &SurroundingText {
        &self.surrounding
    }

    /// Updates surrounding text without weakening secure policy.
    pub fn update_surrounding(&mut self, surrounding: SurroundingText) {
        self.surrounding = if self.purpose == Purpose::Secure {
            SurroundingText::Redacted
        } else {
            surrounding
        };
    }

    /// Updates candidate-window geometry.
    pub fn set_candidate_rect(&mut self, rect: Rectangle) {
        self.candidate = rect;
    }

    /// Returns candidate-window geometry.
    pub const fn candidate_rect(&self) -> Rectangle {
        self.candidate
    }

    /// Advances and returns the wrapping protocol serial.
    pub fn commit_serial(&mut self) -> u32 {
        self.serial = self.serial.wrapping_add(1);
        self.serial
    }

    /// Cancels and redacts the session.
    pub fn cancel(&mut self) -> Event {
        self.enabled = false;
        self.surrounding = SurroundingText::Redacted;
        Event::Cancelled
    }

    /// Returns whether the session accepts events.
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secure_session_never_exposes_supplied_surrounding_text() {
        let visible = SurroundingText::Visible {
            content: "secret".into(),
            selection: 0..6,
        };
        let mut session = Session::new(Purpose::Secure, visible.clone(), Rectangle::default());
        assert!(session.surrounding_text().is_redacted());
        session.update_surrounding(visible);
        assert!(session.surrounding_text().is_redacted());
    }
}
