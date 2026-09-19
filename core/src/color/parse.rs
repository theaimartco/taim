use super::Color;

/// An error which can be returned when parsing color from an RGB hexadecimal string.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// Kept for source compatibility with earlier TAIM releases.
    #[error(transparent)]
    ParseIntError(#[from] std::num::ParseIntError),
    /// The string contains a character that is not an ASCII hexadecimal digit.
    #[error("expected only ASCII hexadecimal digits")]
    InvalidCharacter,
    /// The string is of invalid length.
    #[error("expected hex string of length 3, 4, 6 or 8 excluding optional prefix '#', found {0}")]
    InvalidLength(usize),
}

impl std::str::FromStr for Color {
    type Err = ParseError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let bytes = input.strip_prefix('#').unwrap_or(input).as_bytes();

        #[inline]
        fn nibble(byte: u8) -> Result<u8, ParseError> {
            match byte {
                b'0'..=b'9' => Ok(byte - b'0'),
                b'a'..=b'f' => Ok(byte - b'a' + 10),
                b'A'..=b'F' => Ok(byte - b'A' + 10),
                _ => Err(ParseError::InvalidCharacter),
            }
        }

        #[inline]
        fn short(bytes: &[u8], index: usize) -> Result<f32, ParseError> {
            Ok(f32::from(nibble(bytes[index])? * 17) / 255.0)
        }

        #[inline]
        fn full(bytes: &[u8], index: usize) -> Result<f32, ParseError> {
            Ok(f32::from((nibble(bytes[index])? << 4) | nibble(bytes[index + 1])?) / 255.0)
        }

        match bytes.len() {
            3 => Ok(Color::from_rgb(
                short(bytes, 0)?,
                short(bytes, 1)?,
                short(bytes, 2)?,
            )),
            4 => Ok(Color::from_rgba(
                short(bytes, 0)?,
                short(bytes, 1)?,
                short(bytes, 2)?,
                short(bytes, 3)?,
            )),
            6 => Ok(Color::from_rgb(
                full(bytes, 0)?,
                full(bytes, 2)?,
                full(bytes, 4)?,
            )),
            8 => Ok(Color::from_rgba(
                full(bytes, 0)?,
                full(bytes, 2)?,
                full(bytes, 4)?,
                full(bytes, 6)?,
            )),
            length => Err(ParseError::InvalidLength(length)),
        }
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [r, g, b, a] = self.into_rgba8();

        if self.a == 1.0 {
            write!(formatter, "#{r:02x}{g:02x}{b:02x}")
        } else {
            write!(formatter, "#{r:02x}{g:02x}{b:02x}{a:02x}")
        }
    }
}
