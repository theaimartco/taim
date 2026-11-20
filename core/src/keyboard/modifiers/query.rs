use super::Modifiers;

impl Modifiers {
    /// Returns whether Shift is active.
    #[inline]
    pub const fn shift(self) -> bool {
        self.contains(Self::SHIFT)
    }

    /// Returns whether Control is active.
    #[inline]
    pub const fn control(self) -> bool {
        self.contains(Self::CTRL)
    }

    /// Returns whether Alt/Option is active.
    #[inline]
    pub const fn alt(self) -> bool {
        self.contains(Self::ALT)
    }

    /// Returns whether Logo/Command is active.
    #[inline]
    pub const fn logo(self) -> bool {
        self.contains(Self::LOGO)
    }

    /// Returns whether the platform's primary shortcut modifier is active.
    #[inline]
    pub const fn command(self) -> bool {
        self.contains(Self::COMMAND)
    }

    /// Returns whether the platform's word-jump modifier is active.
    #[inline]
    pub const fn jump(self) -> bool {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        return self.contains(Self::ALT);

        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        return self.contains(Self::CTRL);
    }

    /// Returns whether a macOS Command key is active.
    #[inline]
    pub const fn macos_command(self) -> bool {
        cfg!(target_os = "macos") && self.contains(Self::LOGO)
    }
}
