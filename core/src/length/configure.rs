use crate::Pixels;

use super::{Bounds, Length, Sizing};

impl Length {
    /// Applies a minimum boundary to a flexible length.
    pub fn min(self, min: impl Into<Pixels>) -> Self {
        let min = min.into().0;
        let min = if min.is_finite() { min.max(0.0) } else { 0.0 };

        let sizing = match self {
            Self::Fit | Self::Fluid(_) => Sizing::Fit,
            Self::Fill => Sizing::Fill(1),
            Self::FillPortion(factor) => Sizing::Fill(factor),
            Self::Shrink => Sizing::Shrink,
            Self::Fixed(_) => return self,
            Self::Bounded { bounds, sizing } => {
                return Self::Bounded {
                    bounds: bounds.min(min),
                    sizing,
                };
            }
        };

        Self::Bounded {
            bounds: Bounds::Min(min),
            sizing,
        }
    }

    /// Applies a maximum boundary to a flexible length.
    pub fn max(self, max: impl Into<Pixels>) -> Self {
        let max = max.into().0;
        let max = if max.is_nan() { 0.0 } else { max.max(0.0) };

        let sizing = match self {
            Self::Fit | Self::Fluid(_) => Sizing::Fit,
            Self::Fill => Sizing::Fill(1),
            Self::FillPortion(factor) => Sizing::Fill(factor),
            Self::Shrink => Sizing::Shrink,
            Self::Fixed(_) => return self,
            Self::Bounded { bounds, sizing } => {
                return Self::Bounded {
                    bounds: bounds.max(max),
                    sizing,
                };
            }
        };

        Self::Bounded {
            bounds: Bounds::Max(max),
            sizing,
        }
    }

    /// Returns the relative fill weight consumed by this length.
    #[inline]
    pub fn fill_factor(&self) -> u16 {
        match self {
            Self::Fill => 1,
            Self::FillPortion(factor)
            | Self::Bounded {
                sizing: Sizing::Fill(factor),
                ..
            } => *factor,
            Self::Fluid(_) => 1,
            Self::Shrink | Self::Fit | Self::Fixed(_) | Self::Bounded { .. } => 0,
        }
    }

    /// Returns whether this length consumes remaining space.
    #[inline]
    pub fn is_fill(&self) -> bool {
        self.fill_factor() != 0
    }

    /// Returns whether this length uses intrinsic content size.
    #[inline]
    pub fn is_fit(&self) -> bool {
        matches!(self, Self::Fit)
    }
}
