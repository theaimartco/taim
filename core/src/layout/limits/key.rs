use super::Limits;

/// Compact equality/hash representation of [`Limits`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    min: (u32, u32),
    max: (u32, u32),
    compression: (bool, bool),
}

impl Limits {
    /// Returns a normalized layout-cache key.
    #[inline]
    pub fn key(&self) -> Key {
        Key {
            min: (bits(self.min.width), bits(self.min.height)),
            max: (bits(self.max.width), bits(self.max.height)),
            compression: (self.compression.width, self.compression.height),
        }
    }
}

#[inline]
fn bits(value: f32) -> u32 {
    if value.is_nan() {
        f32::NAN.to_bits()
    } else if value == 0.0 {
        0.0f32.to_bits()
    } else {
        value.to_bits()
    }
}
