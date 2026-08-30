//! Multilingual paragraphs, edits, emoji, and glyph fallback.
//!
//! Phase 1 records the baseline cost of shaping/rendering this corpus
//! with the current, unoptimized text path. Phase 10 (`core/src/text/*`,
//! `wgpu/src/text.rs`) is expected to reduce repeated shaping/uploads for
//! the same content; this scene's `text_edit` variant (typing) is what
//! should show the biggest improvement once shaped-run caching lands.

use crate::scenes::{Element, Scene};

use taim_core::Length;
use taim_widget::{column, container, scrollable, text};

/// Command-line/report identifier for this scene.
pub const NAME: &str = "text_heavy";

/// Static multilingual paragraphs covering phase.md's mandatory language
/// matrix (English, Devanagari, Gurmukhi, Arabic/RTL, Bengali, Tamil,
/// emoji ZWJ sequences, mixed BiDi). These are fixed literals, not a
/// generator, so the rendered content is identical on every run and
/// platform.
const PARAGRAPHS: &[&str] = &[
    "The quick brown fox jumps over the lazy dog. VYOM Taim targets \
     measurable leadership in latency, frame consistency, and efficiency.",
    "यह एक हिंदी अनुच्छेद है जो देवनागरी लिपि का उपयोग करता है। \
     पाठ प्रस्तुतिकरण की गुणवत्ता का परीक्षण करने के लिए इसका उपयोग किया जाता है।",
    "ਇਹ ਇੱਕ ਪੰਜਾਬੀ ਪੈਰਾਗ੍ਰਾਫ ਹੈ ਜੋ ਗੁਰਮੁਖੀ ਲਿਪੀ ਦੀ ਵਰਤੋਂ ਕਰਦਾ ਹੈ। \
     ਇਹ ਟੈਕਸਟ ਰੈਂਡਰਿੰਗ ਗੁਣਵੱਤਾ ਦੀ ਜਾਂਚ ਲਈ ਵਰਤਿਆ ਜਾਂਦਾ ਹੈ।",
    "هذه فقرة عربية تستخدم الكتابة من اليمين إلى اليسار. \
     يتم استخدامها لاختبار جودة عرض النص ثنائي الاتجاه المختلط.",
    "এটি একটি বাংলা অনুচ্ছেদ যা বাংলা লিপি ব্যবহার করে। \
     এটি টেক্সট রেন্ডারিং মানের পরীক্ষা করতে ব্যবহৃত হয়।",
    "இது தமிழ் எழுத்துக்களைப் பயன்படுத்தும் ஒரு தமிழ் பத்தி. \
     இது உரை வழங்கல் தரத்தை சோதிக்கப் பயன்படுத்தப்படுகிறது।",
    "Emoji and ZWJ sequences: 👨‍👩‍👧‍👦 👍🏽 🏳️‍🌈 🇮🇳 🇺🇸 combining marks: e\u{0301}tude, \
     mixed BiDi: The title is كتاب مفتوح (an open book) in translation.",
    "Missing-glyph fallback probe: \u{FFFD} \u{10FFFF} \u{2603}\u{FE0F} \
     should fall back to a substitute glyph rather than panicking.",
];

/// A single character-at-a-time typing edit applied to a scratch buffer,
/// simulating live text input cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// Appends the next character of the scripted edit sequence.
    TypeNextCharacter,
    /// Resets the scratch buffer (simulating "select all, delete").
    Clear,
}

/// A scrollable column of multilingual paragraphs, plus one scratch
/// paragraph being typed into character by character.
#[derive(Debug)]
pub struct TextHeavy {
    edit_source: Vec<char>,
    edit_cursor: usize,
    scratch: String,
}

impl TextHeavy {
    /// Creates the scene. The typed content is `PARAGRAPHS[0]`, so the
    /// "edits" case exercises the same shaping-affecting inputs (Latin
    /// text) as the static paragraphs above it.
    pub fn new() -> Self {
        Self {
            edit_source: PARAGRAPHS[0].chars().collect(),
            edit_cursor: 0,
            scratch: String::new(),
        }
    }
}

impl Default for TextHeavy {
    fn default() -> Self {
        Self::new()
    }
}

impl Scene for TextHeavy {
    type Message = Message;

    fn name(&self) -> &'static str {
        NAME
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let mut paragraphs: Vec<Element<'_, Message>> = PARAGRAPHS
            .iter()
            .map(|paragraph| Element::from(text(*paragraph)))
            .collect();

        paragraphs.push(Element::from(text(format!("(typing) {}", self.scratch))));

        container(
            scrollable(column(paragraphs).width(Length::Fill).spacing(12)).height(Length::Fill),
        )
        .padding(8)
        .into()
    }

    fn update(&mut self, message: Self::Message) {
        match message {
            Message::TypeNextCharacter => {
                if let Some(&next) = self.edit_source.get(self.edit_cursor) {
                    self.scratch.push(next);
                    self.edit_cursor += 1;
                }
            }
            Message::Clear => {
                self.scratch.clear();
                self.edit_cursor = 0;
            }
        }
    }

    fn advance(&mut self, _frame_index: u64) -> Vec<Self::Message> {
        if self.edit_cursor >= self.edit_source.len() {
            return vec![Message::Clear];
        }

        // One character typed per frame; deterministic and independent
        // of `frame_index`'s exact value beyond "has this run already
        // exhausted the source text".
        vec![Message::TypeNextCharacter]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_reaches_the_end_and_then_clears() {
        let mut scene = TextHeavy::new();
        let total = scene.edit_source.len();

        for frame in 0..total as u64 {
            for message in scene.advance(frame) {
                scene.update(message);
            }
        }

        assert_eq!(scene.scratch.chars().count(), total);

        for message in scene.advance(total as u64) {
            scene.update(message);
        }

        assert!(scene.scratch.is_empty());
    }

    #[test]
    fn all_paragraphs_are_non_empty() {
        for paragraph in PARAGRAPHS {
            assert!(!paragraph.is_empty());
        }
    }
}
