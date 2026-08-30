//! Shape tokens.
/// Canonical corner radius values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Radius {
    /// Square/flush corners.
    None,
    /// Small control radius.
    Small,
    /// Standard surface radius.
    #[default]
    Medium,
    /// Large sheets and cards.
    Large,
    /// Fully rounded pill.
    Full,
}
impl Radius {
    /// Returns radius in logical pixels, if finite.
    pub const fn pixels(self) -> Option<f32> {
        match self {
            Self::None => Some(0.0),
            Self::Small => Some(6.0),
            Self::Medium => Some(12.0),
            Self::Large => Some(20.0),
            Self::Full => None,
        }
    }
}
