//! Full editor snapshots for platforms whose IME owns an input connection.
use std::ops::Range;

/// UTF-8 editor state exchanged with a native input connection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EditorState {
    /// Text including the current composition, if present.
    pub content: String,
    /// Selection in UTF-8 bytes, preserving anchor/caret direction.
    pub selection: Range<usize>,
    /// Uncommitted composition range, in UTF-8 bytes.
    pub composing: Option<Range<usize>>,
    /// Whether the editor accepts newlines.
    pub multiline: bool,
}

impl EditorState {
    /// Normalizes untrusted offsets to valid UTF-8 boundaries.
    pub fn normalized(mut self) -> Self {
        self.selection = boundary(&self.content, self.selection.start)
            ..boundary(&self.content, self.selection.end);
        self.composing = self.composing.and_then(|range| {
            let start = boundary(&self.content, range.start);
            let end = boundary(&self.content, range.end);
            (start < end).then_some(start..end)
        });
        self
    }
}

/// Converts an Android UTF-16 offset to a UTF-8 byte offset, rounding down
/// rather than splitting a surrogate pair.
pub fn utf16_to_utf8(text: &str, offset: usize) -> usize {
    let mut units = 0;
    for (index, character) in text.char_indices() {
        if units + character.len_utf16() > offset {
            return index;
        }
        units += character.len_utf16();
        if units == offset {
            return index + character.len_utf8();
        }
    }
    text.len()
}

/// Converts a UTF-8 byte offset to Android UTF-16 units, rounding down at
/// incomplete characters.
pub fn utf8_to_utf16(text: &str, offset: usize) -> usize {
    text[..boundary(text, offset)].encode_utf16().count()
}

fn boundary(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn surrogate_pairs_and_mixed_scripts_never_split() {
        let text = "a💨नमस्ते";
        assert_eq!(utf16_to_utf8(text, 0), 0);
        assert_eq!(utf16_to_utf8(text, 1), 1);
        assert_eq!(utf16_to_utf8(text, 2), 1);
        assert_eq!(utf16_to_utf8(text, 3), 5);
        assert_eq!(utf8_to_utf16(text, 3), 1);
        for (index, _) in text.char_indices() {
            assert_eq!(utf16_to_utf8(text, utf8_to_utf16(text, index)), index);
        }
    }
}
