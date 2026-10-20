//! Listen to keyboard events.
pub mod key;

mod event;
mod location;
mod modifiers;

pub use event::Event;
pub use key::Key;
pub use location::Location;
pub use modifiers::Modifiers;

/// TAIM key-event classification shared by IME-aware widgets and shortcuts.
impl Event {
    /// A repeat is a held-key press, not a new physical key depression.
    #[inline]
    pub fn is_repeat_press(&self) -> bool {
        matches!(self, Self::KeyPressed { repeat: true, .. })
    }

    /// Modifier-only changes should update shortcut state without inserting
    /// characters into focused text editors.
    #[inline]
    pub fn is_modifier_transition(&self) -> bool {
        matches!(self, Self::ModifiersChanged(_))
    }
}

#[cfg(test)]
mod taim_keyboard_event_tests {
    use super::*;

    #[test]
    fn modifiers_never_count_as_repeated_text_input() {
        let event = Event::ModifiersChanged(Modifiers::default());
        assert!(event.is_modifier_transition());
        assert!(!event.is_repeat_press());
    }
}
