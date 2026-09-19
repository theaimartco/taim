//! Hardware-position keys and native fallbacks.

/// A platform-neutral physical keyboard position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(missing_docs)]
#[non_exhaustive]
pub enum Code {
    Backquote,
    Backslash,
    BracketLeft,
    BracketRight,
    Comma,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Equal,
    IntlBackslash,
    IntlRo,
    IntlYen,
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
    Minus,
    Period,
    Quote,
    Semicolon,
    Slash,
    AltLeft,
    AltRight,
    Backspace,
    CapsLock,
    ContextMenu,
    ControlLeft,
    ControlRight,
    Enter,
    SuperLeft,
    SuperRight,
    ShiftLeft,
    ShiftRight,
    Space,
    Tab,
    Convert,
    KanaMode,
    Lang1,
    Lang2,
    Lang3,
    Lang4,
    Lang5,
    NonConvert,
    Delete,
    End,
    Help,
    Home,
    Insert,
    PageDown,
    PageUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    NumLock,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    NumpadAdd,
    NumpadBackspace,
    NumpadClear,
    NumpadClearEntry,
    NumpadComma,
    NumpadDecimal,
    NumpadDivide,
    NumpadEnter,
    NumpadEqual,
    NumpadHash,
    NumpadMemoryAdd,
    NumpadMemoryClear,
    NumpadMemoryRecall,
    NumpadMemoryStore,
    NumpadMemorySubtract,
    NumpadMultiply,
    NumpadParenLeft,
    NumpadParenRight,
    NumpadStar,
    NumpadSubtract,
    Escape,
    Fn,
    FnLock,
    PrintScreen,
    ScrollLock,
    Pause,
    BrowserBack,
    BrowserFavorites,
    BrowserForward,
    BrowserHome,
    BrowserRefresh,
    BrowserSearch,
    BrowserStop,
    Eject,
    LaunchApp1,
    LaunchApp2,
    LaunchMail,
    MediaPlayPause,
    MediaSelect,
    MediaStop,
    MediaTrackNext,
    MediaTrackPrevious,
    Power,
    Sleep,
    AudioVolumeDown,
    AudioVolumeMute,
    AudioVolumeUp,
    WakeUp,
    Meta,
    Hyper,
    Turbo,
    Abort,
    Resume,
    Suspend,
    Again,
    Copy,
    Cut,
    Find,
    Open,
    Paste,
    Props,
    Select,
    Undo,
    Hiragana,
    Katakana,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    F25,
    F26,
    F27,
    F28,
    F29,
    F30,
    F31,
    F32,
    F33,
    F34,
    F35,
}

impl Code {
    pub(super) const fn latin_label(self) -> Option<char> {
        Some(match self {
            Self::KeyA => 'a',
            Self::KeyB => 'b',
            Self::KeyC => 'c',
            Self::KeyD => 'd',
            Self::KeyE => 'e',
            Self::KeyF => 'f',
            Self::KeyG => 'g',
            Self::KeyH => 'h',
            Self::KeyI => 'i',
            Self::KeyJ => 'j',
            Self::KeyK => 'k',
            Self::KeyL => 'l',
            Self::KeyM => 'm',
            Self::KeyN => 'n',
            Self::KeyO => 'o',
            Self::KeyP => 'p',
            Self::KeyQ => 'q',
            Self::KeyR => 'r',
            Self::KeyS => 's',
            Self::KeyT => 't',
            Self::KeyU => 'u',
            Self::KeyV => 'v',
            Self::KeyW => 'w',
            Self::KeyX => 'x',
            Self::KeyY => 'y',
            Self::KeyZ => 'z',
            Self::Digit0 => '0',
            Self::Digit1 => '1',
            Self::Digit2 => '2',
            Self::Digit3 => '3',
            Self::Digit4 => '4',
            Self::Digit5 => '5',
            Self::Digit6 => '6',
            Self::Digit7 => '7',
            Self::Digit8 => '8',
            Self::Digit9 => '9',
            _ => return None,
        })
    }
}

/// Platform-native identity for an otherwise unmapped physical key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NativeCode {
    /// No native identity was provided.
    Unidentified,
    /// Android scan code.
    Android(u32),
    /// macOS scan code.
    MacOS(u16),
    /// Windows scan code.
    Windows(u16),
    /// XKB key code.
    Xkb(u32),
}

/// Known physical position or a platform-native fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Physical {
    /// Standardized position.
    Code(Code),
    /// Unmapped native position.
    Unidentified(NativeCode),
}

impl PartialEq<Code> for Physical {
    #[inline]
    fn eq(&self, other: &Code) -> bool {
        matches!(self, Self::Code(code) if code == other)
    }
}

impl PartialEq<Physical> for Code {
    #[inline]
    fn eq(&self, other: &Physical) -> bool {
        other == self
    }
}

impl PartialEq<NativeCode> for Physical {
    #[inline]
    fn eq(&self, other: &NativeCode) -> bool {
        matches!(self, Self::Unidentified(code) if code == other)
    }
}

impl PartialEq<Physical> for NativeCode {
    #[inline]
    fn eq(&self, other: &Physical) -> bool {
        other == self
    }
}
