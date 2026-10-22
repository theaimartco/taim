//! What a widget asks of the platform input method (IME) each frame.
use crate::{Pixels, Rectangle};
use std::ops::Range;

/// Whether a widget wants the input method, and how.
///
/// The focused text widget reports this every redraw; the shell keeps the
/// first enabled request it receives (see [`merge`](InputMethod::merge)).
#[derive(Debug, Clone, PartialEq)]
pub enum InputMethod<T = String> {
    /// No text entry is focused.
    Disabled,
    /// A text field is focused.
    Enabled {
        /// The caret area. Candidate windows are placed so they do not cover
        /// it.
        cursor: Rectangle,
        /// What kind of text is being entered.
        purpose: Purpose,
        /// Text being composed, for the shell to draw near the caret.
        ///
        /// Widgets that render composition inline ("on the spot") leave this
        /// `None`; widgets that do not can hand it to the shell, which draws
        /// it as an overlay ("over the spot").
        preedit: Option<Preedit<T>>,
    },
}

/// Text an input method is composing but has not committed yet.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Preedit<T = String> {
    /// The composed text.
    pub content: T,
    /// The part of `content` the input method highlights, in bytes.
    pub selection: Option<Range<usize>>,
    /// Font size for the overlay; the renderer default when `None`.
    pub text_size: Option<Pixels>,
}

impl<T> Preedit<T> {
    /// Nothing composed.
    pub fn new() -> Self
    where
        T: Default,
    {
        Self::default()
    }

    /// An owned copy of the composition.
    pub fn to_owned(&self) -> Preedit
    where
        T: AsRef<str>,
    {
        self.with_content(self.content.as_ref().to_owned())
    }

    fn with_content<U>(&self, content: U) -> Preedit<U> {
        Preedit {
            content,
            selection: self.selection.clone(),
            text_size: self.text_size,
        }
    }
}

impl Preedit {
    /// The composition with its text borrowed.
    pub fn as_ref(&self) -> Preedit<&str> {
        self.with_content(self.content.as_str())
    }
}

/// What kind of text is being entered; platforms adapt their keyboards and
/// candidate lists to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Purpose {
    /// Ordinary text.
    #[default]
    Normal,
    /// A secret, such as a password: no suggestions, no learning.
    Secure,
    /// A terminal; on-screen keyboards may offer extra keys.
    Terminal,
}

impl InputMethod {
    /// Takes `other` unless an input method is already enabled: the first
    /// widget to claim the input method keeps it.
    ///
    /// ```
    /// # use taim_core::input_method::{InputMethod, Purpose};
    /// # use taim_core::{Point, Rectangle, Size};
    /// let field = |purpose| InputMethod::<String>::Enabled {
    ///     cursor: Rectangle::new(Point::ORIGIN, Size::UNIT),
    ///     purpose,
    ///     preedit: None,
    /// };
    ///
    /// let mut ime = InputMethod::Disabled;
    /// ime.merge(&field(Purpose::Normal));
    /// ime.merge(&field(Purpose::Secure));
    ///
    /// assert_eq!(ime, field(Purpose::Normal));
    /// ```
    pub fn merge<T: AsRef<str>>(&mut self, other: &InputMethod<T>) {
        if !self.is_enabled() {
            *self = other.to_owned();
        }
    }

    /// Whether a text field claimed the input method.
    #[inline]
    pub const fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled { .. })
    }
}

impl<T> InputMethod<T> {
    /// An owned copy, with any composed text copied.
    pub fn to_owned(&self) -> InputMethod
    where
        T: AsRef<str>,
    {
        let Self::Enabled {
            cursor,
            purpose,
            preedit,
        } = self
        else {
            return InputMethod::Disabled;
        };

        InputMethod::Enabled {
            cursor: *cursor,
            purpose: *purpose,
            preedit: preedit.as_ref().map(Preedit::to_owned),
        }
    }
}
