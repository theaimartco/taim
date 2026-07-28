//! Fused color transform.
/// 4x5 color matrix applied in an existing pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorMatrix {
    pub values: [f32; 20],
}
impl ColorMatrix {
    pub const IDENTITY: Self = Self {
        values: [
            1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0,
        ],
    };
}
