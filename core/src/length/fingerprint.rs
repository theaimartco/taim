use super::{Bounds, Constraint, Length, Sizing};

impl Length {
    /// Writes a stable representation suitable for a layout cache fingerprint.
    pub fn hash_stable(&self, hasher: &mut dyn std::hash::Hasher) {
        fn float(value: f32, hasher: &mut dyn std::hash::Hasher) {
            hasher.write_u32(if value == 0.0 {
                0.0f32.to_bits()
            } else {
                value.to_bits()
            });
        }

        hasher.write_u8(match self {
            Length::Fit => 0,
            Length::Fill => 1,
            Length::FillPortion(_) => 2,
            Length::Shrink => 3,
            Length::Fixed(_) => 4,
            Length::Bounded { .. } => 5,
            Length::Fluid(_) => 6,
        });

        match self {
            Length::Fit | Length::Fill | Length::Shrink => {}
            Length::FillPortion(factor) => hasher.write_u16(*factor),
            Length::Fixed(amount) => float(*amount, hasher),
            Length::Bounded { bounds, sizing } => {
                match bounds {
                    Bounds::Min(min) => {
                        hasher.write_u8(0);
                        float(*min, hasher);
                    }
                    Bounds::Max(max) => {
                        hasher.write_u8(1);
                        float(*max, hasher);
                    }
                    Bounds::Both { min, max } => {
                        hasher.write_u8(2);
                        float(*min, hasher);
                        float(*max, hasher);
                    }
                }

                match sizing {
                    Sizing::Fit => hasher.write_u8(0),
                    Sizing::Fill(factor) => {
                        hasher.write_u8(1);
                        hasher.write_u16(*factor);
                    }
                    Sizing::Shrink => hasher.write_u8(2),
                }
            }
            Length::Fluid(Constraint::Min(min)) => {
                hasher.write_u8(0);
                float(*min, hasher);
            }
            Length::Fluid(Constraint::Max) => hasher.write_u8(1),
        }
    }
}
