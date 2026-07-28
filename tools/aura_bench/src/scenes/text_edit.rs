//! Long-document editing, selection, and shaping-cache churn workload.

use crate::scenes::{Element, Scene};

use aura_core::Length;
use aura_widget::{column, container, scrollable, text, text_input};

/// Command-line/report identifier.
pub const NAME: &str = "text_edit";

const DOCUMENT: &str = "Aura text editing cache workload. \n";
const EDIT_SCRIPT: &[char] = &[
    'न', 'म', 'स', '्', 'त', 'े', ' ', '👋', ' ', 'ع', 'ر', 'ب', 'ي',
];

/// Deterministic edit commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    /// Applies the next edit.
    Edit,
    /// Replaces the scratch value, forcing a bounded cache-key change.
    Replace,
}

/// A text-edit scene with a stable long document and repeatable churn.
#[derive(Debug)]
pub struct TextEdit {
    document: String,
    value: String,
    cursor: usize,
}

impl TextEdit {
    /// Creates the deterministic workload.
    pub fn new() -> Self {
        Self {
            document: DOCUMENT.repeat(256),
            value: String::new(),
            cursor: 0,
        }
    }
}

impl Default for TextEdit {
    fn default() -> Self {
        Self::new()
    }
}

impl Scene for TextEdit {
    type Message = Message;

    fn name(&self) -> &'static str {
        NAME
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let editor = text_input("IME / selection edit", &self.value)
            .on_input(|_| Message::Edit)
            .secure(false);

        container(
            column([
                Element::from(editor),
                Element::from(scrollable(text(&self.document)).height(Length::Fill)),
            ])
            .spacing(12)
            .width(Length::Fill),
        )
        .padding(12)
        .into()
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::Edit => {
                self.value
                    .push(EDIT_SCRIPT[self.cursor % EDIT_SCRIPT.len()]);
                self.cursor += 1;
            }
            Message::Replace => {
                self.value.clear();
                self.value
                    .push_str("selection replacement: தமிழ் বাংলা العربية");
            }
        }
    }

    fn advance(&mut self, frame_index: u64) -> Vec<Message> {
        if frame_index.is_multiple_of(37) {
            vec![Message::Replace]
        } else {
            vec![Message::Edit]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_script_and_document_are_bounded_and_deterministic() {
        let mut scene = TextEdit::new();
        let initial_document_len = scene.document.len();

        for message in scene.advance(1) {
            scene.update(message);
        }

        assert!(!scene.value.is_empty());
        assert_eq!(scene.document.len(), initial_document_len);
    }
}
