use crate::text::{FallbackChain, Script};
use std::hash::{Hash, Hasher};

/// The base direction used to resolve a paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Direction {
    /// Infer from the paragraph's first strong character.
    #[default]
    Auto,
    /// Resolve the paragraph left-to-right.
    LeftToRight,
    /// Resolve the paragraph right-to-left.
    RightToLeft,
}

/// Shaping attributes that must participate in paragraph cache identity.
///
/// This deliberately keeps locale, direction, fallback and variable-font axis
/// identity together. A renderer can translate it to backend-specific font
/// attributes without accidentally reusing a run shaped for incompatible
/// inputs.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShapingKey {
    /// BCP-47 locale when the caller knows it.
    pub locale: Option<String>,
    /// Paragraph base direction.
    pub direction: Direction,
    /// Script chosen by application policy or detection.
    pub script: Script,
    /// Ordered fallback family chain.
    pub fallback: FallbackChain,
    /// Quantized variable-font axes, sorted by four-byte axis tag.
    pub axes: Vec<([u8; 4], u32)>,
}

impl Default for ShapingKey {
    fn default() -> Self {
        Self {
            locale: None,
            direction: Direction::Auto,
            script: Script::Latin,
            fallback: FallbackChain::for_script(Script::Latin),
            axes: Vec::new(),
        }
    }
}

impl ShapingKey {
    /// Creates a default key after deterministic script detection.
    pub fn for_text(text: &str) -> Self {
        let script = text
            .chars()
            .map(Script::detect)
            .find(|script| !matches!(script, Script::Latin | Script::Other))
            .unwrap_or(Script::Latin);

        Self {
            script,
            fallback: FallbackChain::for_script(script),
            ..Self::default()
        }
    }

    /// Sets a BCP-47 locale.
    pub fn locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = Some(locale.into());
        self
    }

    /// Sets the paragraph direction.
    pub fn direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    /// Adds or replaces a quantized variable-font axis.
    pub fn axis(mut self, tag: [u8; 4], value: f32) -> Self {
        let value = value.to_bits();

        match self.axes.binary_search_by_key(&tag, |axis| axis.0) {
            Ok(index) => self.axes[index].1 = value,
            Err(index) => self.axes.insert(index, (tag, value)),
        }

        self
    }

    /// Hashes the full shaping identity into a caller-provided hasher.
    pub fn hash_into(&self, state: &mut impl Hasher) {
        self.hash(state);
    }
}
