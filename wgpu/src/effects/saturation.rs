//! Saturation specialisation shared with color matrix passes.
/// Saturation multiplier where zero is grayscale and one is identity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Saturation(pub f32);
