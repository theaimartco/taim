//! Text from an input method that edits a buffer of its own.
//!
//! Android's input methods (through GameActivity's text input) do not send
//! keystrokes: they keep a text buffer and rewrite it as the user types,
//! composes, accepts a suggestion or swipes a word. TAIM's text views take
//! typed text and Backspace, so the buffer's changes are replayed as those:
//! the characters that differ from what was typed last are deleted and the
//! new ones typed. Composing text therefore shows as it is composed, and an
//! autocorrection replaces the word it corrects.

/// One change to replay: delete `delete` characters before the caret, then
/// type `insert`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Edit {
    pub delete: usize,
    pub insert: String,
}

/// What has been typed into the focused view from the input method's buffer.
#[derive(Debug, Default)]
pub(crate) struct Typing {
    typed: String,
}

impl Typing {
    /// The edit that turns what was typed into `buffer`, if any.
    pub fn update(&mut self, buffer: &str) -> Option<Edit> {
        if buffer == self.typed {
            return None;
        }

        let common: usize = self
            .typed
            .chars()
            .zip(buffer.chars())
            .take_while(|(typed, wanted)| typed == wanted)
            .map(|(typed, _)| typed.len_utf8())
            .sum();

        let edit = Edit {
            delete: self.typed[common..].chars().count(),
            insert: buffer[common..].to_owned(),
        };
        buffer.clone_into(&mut self.typed);
        Some(edit)
    }

    /// Starts afresh: the buffer was emptied, or the caret moved.
    pub fn reset(&mut self) {
        self.typed.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(delete: usize, insert: &str) -> Option<Edit> {
        Some(Edit {
            delete,
            insert: insert.to_owned(),
        })
    }

    #[test]
    fn typing_appends() {
        let mut typing = Typing::default();

        assert_eq!(typing.update("h"), edit(0, "h"));
        assert_eq!(typing.update("he"), edit(0, "e"));
        assert_eq!(typing.update("he"), None, "no change, no edit");
    }

    #[test]
    fn an_autocorrection_replaces_only_what_changed() {
        let mut typing = Typing::default();
        let _ = typing.update("teh");

        assert_eq!(typing.update("the "), edit(2, "he "));
    }

    #[test]
    fn deleting_from_the_buffer_deletes_from_the_view() {
        let mut typing = Typing::default();
        let _ = typing.update("hello");

        assert_eq!(typing.update("hel"), edit(2, ""));
    }

    #[test]
    fn composition_in_devanagari_counts_characters_not_bytes() {
        // Typing "नमस्ते" with a transliterating keyboard rewrites the
        // composing word as it grows.
        let mut typing = Typing::default();
        let _ = typing.update("नम");

        assert_eq!(typing.update("नमस"), edit(0, "स"));
        assert_eq!(typing.update("नमस्ते"), edit(0, "्ते"));
        assert_eq!(
            typing.update("नमस्कार"),
            edit(2, "कार"),
            "two code points differ: the virama stays, त and the vowel sign go"
        );
    }

    #[test]
    fn reset_starts_from_an_empty_buffer() {
        let mut typing = Typing::default();
        let _ = typing.update("word ");
        typing.reset();

        assert_eq!(typing.update("next"), edit(0, "next"));
    }
}
