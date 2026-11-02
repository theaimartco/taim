use unicode_segmentation::UnicodeSegmentation;

/// The value of a [`TextInput`].
///
/// # Security
///
/// A [`Value`] holds whatever the user typed, which may be a password,
/// a recovery phrase, or other secret. It therefore **never** discloses
/// its contents through [`std::fmt::Debug`]: a derived `Debug` would dump
/// the plaintext into every log line, panic message, crash report, and
/// profiler capture that happened to format a struct containing one --
/// exactly the "debug logs" disclosure channel `phase.md` Phase 10's
/// security requirements forbid.
///
/// The type cannot know whether the field it belongs to is secure (that
/// is [`TextInput::secure`](super::TextInput::secure)'s flag, one layer
/// up), so it must default to denying disclosure. Code that genuinely
/// needs the content calls [`Value::to_string`] explicitly, which is an
/// auditable choice at the call site rather than an accident of
/// formatting.
///
/// [`TextInput`]: super::TextInput
#[derive(Clone)]
pub struct Value {
    text: String,
    /// UTF-8 byte offsets for every grapheme plus the terminal text length.
    /// Keeping one contiguous buffer avoids a heap allocation per grapheme.
    boundaries: Vec<usize>,
}

impl std::fmt::Debug for Value {
    /// Formats without disclosing content *or* length.
    ///
    /// Length is deliberately withheld too: this crate already holds that
    /// "bullets or a length-preserving mask ... still disclose[]
    /// information" (see
    /// [`input_method::SurroundingText`](taim_core::input_method::SurroundingText)),
    /// and a password's length is exactly the kind of thing that
    /// principle exists to protect. Call [`Value::len`] if you need it.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Value(<redacted>)")
    }
}

impl Value {
    /// Creates a new [`Value`] from a string slice.
    pub fn new(string: &str) -> Self {
        let text = string.to_owned();
        let boundaries = Self::boundaries(&text);

        Self { text, boundaries }
    }

    /// Returns whether the [`Value`] is empty or not.
    ///
    /// A [`Value`] is empty when it contains no graphemes.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the total amount of graphemes in the [`Value`].
    pub fn len(&self) -> usize {
        self.boundaries.len().saturating_sub(1)
    }

    /// Returns the position of the previous start of a word from the given
    /// grapheme `index`.
    pub fn previous_start_of_word(&self, index: usize) -> usize {
        let byte = self.byte_index(index);
        let previous = &self.text[..byte];

        UnicodeSegmentation::split_word_bound_indices(previous)
            .rfind(|(_, word)| !word.trim_start().is_empty())
            .map(|(byte, _word)| self.grapheme_at_byte(byte))
            .unwrap_or(0)
    }

    /// Returns the position of the next end of a word from the given grapheme
    /// `index`.
    pub fn next_end_of_word(&self, index: usize) -> usize {
        let byte = self.byte_index(index);
        let following = &self.text[byte..];

        UnicodeSegmentation::split_word_bound_indices(following)
            .find(|(_, word)| !word.trim_start().is_empty())
            .map(|(offset, word)| self.grapheme_at_byte(byte + offset + word.len()))
            .unwrap_or(self.len())
    }

    /// Returns a new [`Value`] containing the graphemes from `start` until the
    /// given `end`.
    pub fn select(&self, start: usize, end: usize) -> Self {
        let start = self.byte_index(start);
        let end = self.byte_index(end);

        Self::new(&self.text[start..end])
    }

    /// Returns a new [`Value`] containing the graphemes until the given
    /// `index`.
    pub fn until(&self, index: usize) -> Self {
        Self::new(&self.text[..self.byte_index(index)])
    }

    /// Inserts a new `char` at the given grapheme `index`.
    pub fn insert(&mut self, index: usize, c: char) {
        self.text.insert(self.byte_index(index), c);
        self.reindex();
    }

    /// Inserts a bunch of graphemes at the given grapheme `index`.
    pub fn insert_many(&mut self, index: usize, value: Value) {
        if value.text.is_empty() {
            return;
        }
        self.text.insert_str(self.byte_index(index), &value.text);
        self.reindex();
    }

    /// Removes the grapheme at the given `index`.
    pub fn remove(&mut self, index: usize) {
        let start = self.boundaries[index];
        let end = self.boundaries[index + 1];
        self.text.replace_range(start..end, "");
        self.reindex();
    }

    /// Removes the graphemes from `start` to `end`.
    pub fn remove_many(&mut self, start: usize, end: usize) {
        if start == end {
            return;
        }
        let start = self.boundaries[start];
        let end = self.boundaries[end];
        self.text.replace_range(start..end, "");
        self.reindex();
    }

    /// Returns a new [`Value`] with all its graphemes replaced with the
    /// dot ('•') character.
    pub fn secure(&self) -> Self {
        Self::new(&"•".repeat(self.len()))
    }

    pub(super) fn as_str(&self) -> &str {
        &self.text
    }

    fn byte_index(&self, grapheme: usize) -> usize {
        self.boundaries[grapheme.min(self.len())]
    }

    #[inline]
    fn grapheme_at_byte(&self, byte: usize) -> usize {
        self.boundaries
            .partition_point(|&start| start <= byte)
            .saturating_sub(1)
    }

    fn reindex(&mut self) {
        // Keep the buffer across keystrokes. A text field usually changes a
        // character at a time, so reallocation per edit costs more than the
        // grapheme scan itself for short UI strings.
        self.boundaries.clear();
        self.boundaries.extend(
            UnicodeSegmentation::grapheme_indices(self.text.as_str(), true)
                .map(|(index, _)| index),
        );
        self.boundaries.push(self.text.len());
    }

    fn boundaries(text: &str) -> Vec<usize> {
        let mut boundaries = UnicodeSegmentation::grapheme_indices(text, true)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();

        boundaries.push(text.len());
        boundaries
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::Value;

    #[test]
    fn editing_reuses_boundary_storage_and_handles_combining_marks() {
        let mut value = Value::new("ábc");
        value.boundaries.reserve(16);
        let capacity = value.boundaries.capacity();

        value.insert(1, 'x');
        assert_eq!(value.len(), 4);
        value.remove_many(1, 3);
        assert_eq!(value.as_str(), "ác");
        assert_eq!(value.len(), 2);
        assert_eq!(value.boundaries.capacity(), capacity);
    }

    #[test]
    fn word_navigation_uses_existing_grapheme_index() {
        let value = Value::new("é blue 🦀");
        assert_eq!(value.previous_start_of_word(value.len()), 7);
        assert_eq!(value.next_end_of_word(0), 1);
        assert_eq!(value.next_end_of_word(2), 6);
    }
}
