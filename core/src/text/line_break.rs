//! Locale-aware line-break policy hooks.

/// A permitted line break in UTF-8 byte offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineBreak {
    /// Byte offset immediately after the breaking character.
    pub offset: usize,
    /// Whether a renderer may display a hyphen at this break.
    pub hyphenate: bool,
}

/// Policy used to derive legal wrapping opportunities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LineBreakPolicy {
    /// Break at Unicode whitespace and common punctuation only.
    #[default]
    Conservative,
    /// Permit character-level breaks for narrow layouts.
    Anywhere,
}

impl LineBreakPolicy {
    /// Produces deterministic, UTF-8-safe break opportunities.
    pub fn opportunities(self, text: &str, _locale: Option<&str>) -> Vec<LineBreak> {
        let mut result = Vec::new();

        for (offset, character) in text.char_indices() {
            let end = offset + character.len_utf8();
            let allowed = match self {
                Self::Conservative => {
                    character.is_whitespace() || matches!(character, '-' | '/' | ',' | ';')
                }
                Self::Anywhere => !character.is_control(),
            };

            if allowed {
                result.push(LineBreak {
                    offset: end,
                    hyphenate: character == '-',
                });
            }
        }

        result
    }
}
