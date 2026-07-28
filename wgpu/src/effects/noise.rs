//! Deterministic material noise.
/// A seeded low-cost noise overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Noise {
    pub seed: u32,
}
