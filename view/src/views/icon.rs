//! Glyphs from the TAIM icon font.
use super::text::{Text, text};

/// One glyph of the TAIM icon font (`Taim-Icons`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Icon(char);

impl Icon {
    /// The icon font.
    pub const FONT: taim_core::Font = taim_core::Font::new("Taim-Icons");

    /// Add.
    pub const ADD: Self = Self('\u{e900}');

    /// Arrow down.
    pub const ARROW_DOWN: Self = Self('\u{e901}');

    /// Arrow left.
    pub const ARROW_LEFT: Self = Self('\u{e902}');

    /// Arrow right.
    pub const ARROW_RIGHT: Self = Self('\u{e903}');

    /// Arrow up.
    pub const ARROW_UP: Self = Self('\u{e904}');

    /// Calendar.
    pub const CALENDAR: Self = Self('\u{e905}');

    /// Check.
    pub const CHECK: Self = Self('\u{e906}');

    /// Chevron down.
    pub const CHEVRON_DOWN: Self = Self('\u{e907}');

    /// Chevron left.
    pub const CHEVRON_LEFT: Self = Self('\u{e908}');

    /// Chevron right.
    pub const CHEVRON_RIGHT: Self = Self('\u{e909}');

    /// Chevron up.
    pub const CHEVRON_UP: Self = Self('\u{e90a}');

    /// Clock.
    pub const CLOCK: Self = Self('\u{e90b}');

    /// Close.
    pub const CLOSE: Self = Self('\u{e90c}');

    /// Delete.
    pub const DELETE: Self = Self('\u{e90d}');

    /// Download.
    pub const DOWNLOAD: Self = Self('\u{e90e}');

    /// Edit.
    pub const EDIT: Self = Self('\u{e90f}');

    /// Filter.
    pub const FILTER: Self = Self('\u{e910}');

    /// Folder.
    pub const FOLDER: Self = Self('\u{e911}');

    /// Heart.
    pub const HEART: Self = Self('\u{e912}');

    /// Home.
    pub const HOME: Self = Self('\u{e913}');

    /// Info.
    pub const INFO: Self = Self('\u{e914}');

    /// Lock.
    pub const LOCK: Self = Self('\u{e915}');

    /// Mail.
    pub const MAIL: Self = Self('\u{e916}');

    /// Menu.
    pub const MENU: Self = Self('\u{e917}');

    /// More horiz.
    pub const MORE_HORIZ: Self = Self('\u{e918}');

    /// More vert.
    pub const MORE_VERT: Self = Self('\u{e919}');

    /// Pause.
    pub const PAUSE: Self = Self('\u{e91a}');

    /// Person.
    pub const PERSON: Self = Self('\u{e91b}');

    /// Play.
    pub const PLAY: Self = Self('\u{e91c}');

    /// Remove.
    pub const REMOVE: Self = Self('\u{e91d}');

    /// Search.
    pub const SEARCH: Self = Self('\u{e91e}');

    /// Settings.
    pub const SETTINGS: Self = Self('\u{e91f}');

    /// Star.
    pub const STAR: Self = Self('\u{e920}');

    /// Upload.
    pub const UPLOAD: Self = Self('\u{e921}');

    /// Visibility.
    pub const VISIBILITY: Self = Self('\u{e922}');

    /// Warning.
    pub const WARNING: Self = Self('\u{e923}');

    /// The glyph's code point in [`FONT`](Self::FONT).
    pub const fn codepoint(self) -> char {
        self.0
    }
}

/// Shows `icon` at the default text size; use [`Text::size`] to resize.
pub fn icon(icon: Icon) -> Text {
    text(icon.0.to_string()).font(Icon::FONT)
}
