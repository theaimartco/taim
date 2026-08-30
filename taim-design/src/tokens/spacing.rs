//! Taim's compact, canonical spacing scale.
/// Logical spacing tokens in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Spacing;
impl Spacing {
    /// 4 px micro gap.
    pub const XS: f32 = 4.0;
    /// 8 px compact gap.
    pub const SM: f32 = 8.0;
    /// 12 px regular gap.
    pub const MD: f32 = 12.0;
    /// 16 px section gap.
    pub const LG: f32 = 16.0;
    /// 24 px major gap.
    pub const XL: f32 = 24.0;
    /// 32 px page gap.
    pub const XXL: f32 = 32.0;
}
