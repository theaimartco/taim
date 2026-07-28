//! Mask/clip composition contract.
/// A bounded mask reference supplied by the layer atlas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mask {
    pub atlas_slot: u32,
}
