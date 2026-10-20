//! Load and use fonts.
use crate::core::Pixels;
use crate::core::font::{Error, Family, Font};
use crate::futures::futures::channel::oneshot;
use crate::task::{self, Task};

use std::borrow::Cow;
use std::fmt;

/// A font action.
pub enum Action {
    /// Load a font from its bytes.
    Load {
        /// The bytes of the font to load.
        bytes: Cow<'static, [u8]>,
        /// The channel to send back the load result.
        channel: oneshot::Sender<Result<(), Error>>,
    },

    /// Lists all system font families.
    List {
        /// The channel to send back the list result.
        channel: oneshot::Sender<Result<Vec<Family>, Error>>,
    },

    /// Sets the new font defaults for the running application.
    SetDefaults {
        /// The new default [`Font`].
        font: Font,
        /// The new default text size.
        text_size: Pixels,
    },
}

impl fmt::Debug for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Load { .. } => f.write_str("Load"),
            Self::List { .. } => f.write_str("List"),
            Self::SetDefaults { font, text_size } => f
                .debug_struct("SetDefaults")
                .field("font", font)
                .field("text_size", text_size)
                .finish(),
        }
    }
}

/// Load a font from its bytes.
pub fn load(bytes: impl Into<Cow<'static, [u8]>>) -> Task<Result<(), Error>> {
    task::oneshot(|channel| {
        crate::Action::Font(Action::Load {
            bytes: bytes.into(),
            channel,
        })
    })
}

/// Lists all the available font families in the system.
pub fn list() -> Task<Result<Vec<Family>, Error>> {
    task::oneshot(|channel| crate::Action::Font(Action::List { channel }))
}

/// Sets a new default [`Font`] and text size for the running application.
pub fn set_defaults<Message>(font: Font, text_size: impl Into<Pixels>) -> Task<Message> {
    task::effect(crate::Action::Font(Action::SetDefaults {
        font,
        // Protect renderer glyph metrics from negative or non-finite sizes.
        text_size: text_size.into().normalized(),
    }))
}

#[cfg(test)]
mod taim_font_size_tests {
    use crate::core::Pixels;

    #[test]
    fn renderer_default_size_validation_is_bounded() {
        assert_eq!(Pixels(12.0).normalized(), Pixels(12.0));
        assert_eq!(Pixels(-1.0).normalized(), Pixels::ZERO);
        assert_eq!(Pixels(f32::NAN).normalized(), Pixels::ZERO);
        assert_eq!(Pixels(f32::INFINITY).normalized(), Pixels::ZERO);
    }
}
