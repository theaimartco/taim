use crate::Rectangle;
use std::ops::Range;

/// Text that may—or deliberately may not—cross the widget/platform boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurroundingText {
    /// Public UTF-8 context and its byte selection.
    Visible {
        /// UTF-8 text around the cursor.
        content: String,
        /// Valid UTF-8 byte selection inside `content`.
        selection: Range<usize>,
    },
    /// A secure field: no content or length leaves the widget.
    Redacted,
}

impl SurroundingText {
    /// Builds disclosure-safe IME context. Invalid byte offsets are rounded
    /// down to UTF-8 boundaries before they can reach a platform API.
    pub fn for_field(content: &str, selection: Range<usize>, secure: bool) -> Self {
        if secure {
            return Self::Redacted;
        }

        let start = floor_boundary(content, selection.start);
        let end = floor_boundary(content, selection.end);
        let selection = start.min(end)..start.max(end);

        Self::Visible {
            content: content.into(),
            selection,
        }
    }

    /// Returns whether content disclosure is disabled.
    #[inline]
    pub const fn is_redacted(&self) -> bool {
        matches!(self, Self::Redacted)
    }
}

#[inline]
fn floor_boundary(text: &str, requested: usize) -> usize {
    let mut index = requested.min(text.len());
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// Logical area a platform should keep clear for its candidate window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CandidateRect {
    /// Logical rectangle the candidate window should avoid covering.
    pub bounds: Rectangle,
}

#[cfg(test)]
mod tests {
    use super::SurroundingText;

    #[test]
    fn secure_context_never_discloses_content_or_length() {
        assert_eq!(
            SurroundingText::for_field("secret", 0..6, true),
            SurroundingText::Redacted
        );
    }

    #[test]
    fn invalid_offsets_are_normalized_to_utf8_boundaries() {
        let context = SurroundingText::for_field("a💨z", 2..99, false);
        assert_eq!(
            context,
            SurroundingText::Visible {
                content: "a💨z".into(),
                selection: 1..6,
            }
        );
    }
}
