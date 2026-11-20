use bitflags::bitflags;

mod query;

bitflags! {
    /// The current state of the keyboard modifiers.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct Modifiers: u32{
        /// The "shift" key.
        const SHIFT = 0b100;
        // const LSHIFT = 0b010 << 0;
        // const RSHIFT = 0b001 << 0;
        //
        /// The "control" key.
        const CTRL = 0b100 << 3;
        // const LCTRL = 0b010 << 3;
        // const RCTRL = 0b001 << 3;
        //
        /// The "alt" key.
        const ALT = 0b100 << 6;
        // const LALT = 0b010 << 6;
        // const RALT = 0b001 << 6;
        //
        /// The "windows" key on Windows, "command" key on Mac, and
    /// "super" key on Linux.
        const LOGO = 0b100 << 9;
        // const LLOGO = 0b010 << 9;
        // const RLOGO = 0b001 << 9;
        /// No modifiers
        const NONE = 0;
    }
}

impl Modifiers {
    /// The "command" key.
    ///
    /// This is normally the main modifier to be used for hotkeys.
    ///
    /// On macOS and iOS hardware keyboards, this is equivalent to `Self::LOGO`.
    /// Otherwise, this is equivalent to `Self::CTRL`.
    pub const COMMAND: Self = if cfg!(any(target_os = "macos", target_os = "ios")) {
        Self::LOGO
    } else {
        Self::CTRL
    };
}
