//! Android key identities retained by winit, converted back for native dispatch.
use winit::keyboard::{KeyCode, NativeKeyCode, PhysicalKey};
pub(crate) fn android_key(key: PhysicalKey) -> Option<i32> {
    match key {
        PhysicalKey::Unidentified(NativeKeyCode::Android(code)) => i32::try_from(code).ok(),
        PhysicalKey::Code(key) => Some(match key {
            KeyCode::KeyA => 29,
            KeyCode::KeyB => 30,
            KeyCode::KeyC => 31,
            KeyCode::KeyD => 32,
            KeyCode::KeyE => 33,
            KeyCode::KeyF => 34,
            KeyCode::KeyG => 35,
            KeyCode::KeyH => 36,
            KeyCode::KeyI => 37,
            KeyCode::KeyJ => 38,
            KeyCode::KeyK => 39,
            KeyCode::KeyL => 40,
            KeyCode::KeyM => 41,
            KeyCode::KeyN => 42,
            KeyCode::KeyO => 43,
            KeyCode::KeyP => 44,
            KeyCode::KeyQ => 45,
            KeyCode::KeyR => 46,
            KeyCode::KeyS => 47,
            KeyCode::KeyT => 48,
            KeyCode::KeyU => 49,
            KeyCode::KeyV => 50,
            KeyCode::KeyW => 51,
            KeyCode::KeyX => 52,
            KeyCode::KeyY => 53,
            KeyCode::KeyZ => 54,
            KeyCode::Digit0 => 7,
            KeyCode::Digit1 => 8,
            KeyCode::Digit2 => 9,
            KeyCode::Digit3 => 10,
            KeyCode::Digit4 => 11,
            KeyCode::Digit5 => 12,
            KeyCode::Digit6 => 13,
            KeyCode::Digit7 => 14,
            KeyCode::Digit8 => 15,
            KeyCode::Digit9 => 16,
            KeyCode::Numpad0 => 144,
            KeyCode::Numpad1 => 145,
            KeyCode::Numpad2 => 146,
            KeyCode::Numpad3 => 147,
            KeyCode::Numpad4 => 148,
            KeyCode::Numpad5 => 149,
            KeyCode::Numpad6 => 150,
            KeyCode::Numpad7 => 151,
            KeyCode::Numpad8 => 152,
            KeyCode::Numpad9 => 153,
            KeyCode::NumpadAdd => 157,
            KeyCode::NumpadSubtract => 156,
            KeyCode::NumpadMultiply => 155,
            KeyCode::NumpadDivide => 154,
            KeyCode::NumpadEnter => 160,
            KeyCode::NumpadEqual => 161,
            KeyCode::NumpadComma => 159,
            KeyCode::NumpadDecimal => 158,
            KeyCode::NumLock => 143,
            KeyCode::ArrowLeft => 21,
            KeyCode::ArrowRight => 22,
            KeyCode::ArrowUp => 19,
            KeyCode::ArrowDown => 20,
            KeyCode::F1 => 131,
            KeyCode::F2 => 132,
            KeyCode::F3 => 133,
            KeyCode::F4 => 134,
            KeyCode::F5 => 135,
            KeyCode::F6 => 136,
            KeyCode::F7 => 137,
            KeyCode::F8 => 138,
            KeyCode::F9 => 139,
            KeyCode::F10 => 140,
            KeyCode::F11 => 141,
            KeyCode::F12 => 142,
            KeyCode::Space => 62,
            KeyCode::Escape => 111,
            KeyCode::Enter => 66,
            KeyCode::Tab => 61,
            KeyCode::PageUp => 92,
            KeyCode::PageDown => 93,
            KeyCode::Home => 122,
            KeyCode::End => 123,
            KeyCode::Insert => 124,
            KeyCode::Backspace => 67,
            KeyCode::Delete => 112,
            KeyCode::Copy => 278,
            KeyCode::Paste => 279,
            KeyCode::Cut => 277,
            KeyCode::AudioVolumeUp => 24,
            KeyCode::AudioVolumeDown => 25,
            KeyCode::AudioVolumeMute => 164,
            KeyCode::MediaPlayPause => 85,
            KeyCode::MediaStop => 86,
            KeyCode::MediaTrackNext => 87,
            KeyCode::MediaTrackPrevious => 88,
            KeyCode::Equal => 70,
            KeyCode::Minus => 69,
            KeyCode::Semicolon => 74,
            KeyCode::Slash => 76,
            KeyCode::Backslash => 73,
            KeyCode::Comma => 55,
            KeyCode::Period => 56,
            KeyCode::Quote => 75,
            KeyCode::Backquote => 68,
            KeyCode::PrintScreen => 120,
            KeyCode::Pause => 121,
            KeyCode::ScrollLock => 116,
            KeyCode::IntlYen => 216,
            KeyCode::Lang1 => 218,
            KeyCode::KanaMode => 215,
            KeyCode::ControlLeft => 113,
            KeyCode::ControlRight => 114,
            KeyCode::ShiftLeft => 59,
            KeyCode::ShiftRight => 60,
            KeyCode::AltLeft => 57,
            KeyCode::AltRight => 58,
            KeyCode::SuperLeft => 117,
            KeyCode::SuperRight => 118,
            KeyCode::BracketLeft => 71,
            KeyCode::BracketRight => 72,
            KeyCode::Power => 26,
            KeyCode::Sleep => 223,
            KeyCode::WakeUp => 224,
            _ => return None,
        }),
        PhysicalKey::Unidentified(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editing_and_shortcut_keys_preserve_android_identity() {
        assert_eq!(android_key(PhysicalKey::Code(KeyCode::Equal)), Some(70));
        assert_eq!(android_key(PhysicalKey::Code(KeyCode::Backspace)), Some(67));
        assert_eq!(android_key(PhysicalKey::Code(KeyCode::Delete)), Some(112));
        assert_eq!(android_key(PhysicalKey::Code(KeyCode::ArrowLeft)), Some(21));
        assert_eq!(
            android_key(PhysicalKey::Code(KeyCode::ControlLeft)),
            Some(113)
        );
        assert_eq!(android_key(PhysicalKey::Code(KeyCode::KeyA)), Some(29));
        assert_eq!(
            android_key(PhysicalKey::Code(KeyCode::NumpadEnter)),
            Some(160)
        );
    }
    #[test]
    fn unknown_android_keys_are_preserved_without_fabricating_other_platform_keys() {
        assert_eq!(
            android_key(PhysicalKey::Unidentified(NativeKeyCode::Android(289))),
            Some(289)
        );
        assert_eq!(
            android_key(PhysicalKey::Unidentified(NativeKeyCode::Xkb(29))),
            None
        );
        assert_eq!(android_key(PhysicalKey::Code(KeyCode::F35)), None);
    }
}
