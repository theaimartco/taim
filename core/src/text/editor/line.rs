//! Borrowed editor lines and explicit line terminators.

use std::borrow::Cow;

/// A borrowed or owned logical line.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Line<'a> {
    /// Text without the terminator.
    pub text: Cow<'a, str>,
    /// Original terminator.
    pub ending: LineEnding,
}

/// A recognized line terminator.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub enum LineEnding {
    /// POSIX `\n`.
    #[default]
    Lf,
    /// Windows `\r\n`.
    CrLf,
    /// Legacy `\r`.
    Cr,
    /// Legacy `\n\r`.
    LfCr,
    /// No terminator.
    None,
}

impl LineEnding {
    /// Returns the exact terminator text.
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
            Self::Cr => "\r",
            Self::LfCr => "\n\r",
            Self::None => "",
        }
    }

    /// Returns the terminator length in bytes.
    #[inline]
    pub const fn len(self) -> usize {
        self.as_str().len()
    }

    /// Returns whether the line has no terminator.
    #[inline]
    pub const fn is_empty(self) -> bool {
        matches!(self, Self::None)
    }
}
