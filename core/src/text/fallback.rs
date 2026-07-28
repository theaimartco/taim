//! Deterministic script detection and fallback ordering.

/// A script class used to select a deterministic fallback chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Script {
    /// Latin and common punctuation.
    #[default]
    Latin,
    /// Devanagari, including Hindi text.
    Devanagari,
    /// Gurmukhi, including Punjabi text.
    Gurmukhi,
    /// Arabic and Arabic-derived writing systems.
    Arabic,
    /// Bengali.
    Bengali,
    /// Tamil.
    Tamil,
    /// Emoji and pictographic sequences.
    Emoji,
    /// A script not covered by Aura's deterministic baseline.
    Other,
}

impl Script {
    /// Detects a script class for one scalar value.
    pub fn detect(character: char) -> Self {
        match character as u32 {
            0x0900..=0x097F => Self::Devanagari,
            0x0A00..=0x0A7F => Self::Gurmukhi,
            0x0600..=0x06FF | 0x0750..=0x077F | 0x08A0..=0x08FF => Self::Arabic,
            0x0980..=0x09FF => Self::Bengali,
            0x0B80..=0x0BFF => Self::Tamil,
            0x1F000..=0x1FAFF | 0x2600..=0x27BF => Self::Emoji,
            value if character.is_ascii() || (0x00C0..=0x024F).contains(&value) => Self::Latin,
            _ => Self::Other,
        }
    }
}

/// Stable ordered font-family names for a script.
///
/// The renderer attempts the application's requested font before these names;
/// this type deliberately contains no platform font database lookups, keeping
/// cache keys reproducible across devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FallbackChain {
    families: &'static [&'static str],
}

impl FallbackChain {
    /// Returns Aura's deterministic family order for `script`.
    pub const fn for_script(script: Script) -> Self {
        let families: &'static [&'static str] = match script {
            Script::Latin => &["Noto Sans", "sans-serif"],
            Script::Devanagari => &["Noto Sans Devanagari", "Noto Sans", "sans-serif"],
            Script::Gurmukhi => &["Noto Sans Gurmukhi", "Noto Sans", "sans-serif"],
            Script::Arabic => &["Noto Naskh Arabic", "Noto Sans Arabic", "sans-serif"],
            Script::Bengali => &["Noto Sans Bengali", "Noto Sans", "sans-serif"],
            Script::Tamil => &["Noto Sans Tamil", "Noto Sans", "sans-serif"],
            Script::Emoji => &["Noto Color Emoji", "emoji"],
            Script::Other => &["Noto Sans", "sans-serif"],
        };

        Self { families }
    }

    /// Returns the ordered family names.
    pub const fn families(self) -> &'static [&'static str] {
        self.families
    }
}
