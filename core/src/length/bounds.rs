/// An open-ended limit used by [`super::Length::Fluid`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Constraint {
    /// Fill available space without shrinking below this value.
    Min(f32),
    /// Fill available space without growing past intrinsic content size.
    Max,
}

impl Constraint {
    pub(super) fn stack(self, other: Self) -> Self {
        match (self, other) {
            (Self::Min(a), Self::Min(b)) => Self::Min(a + b),
            (Self::Min(min), Self::Max) | (Self::Max, Self::Min(min)) => Self::Min(min),
            (Self::Max, Self::Max) => Self::Max,
        }
    }

    pub(super) fn cross(self, other: Self) -> Self {
        match (self, other) {
            (Self::Min(a), Self::Min(b)) => Self::Min(a.max(b)),
            (Self::Min(min), Self::Max) | (Self::Max, Self::Min(min)) => Self::Min(min),
            (Self::Max, Self::Max) => Self::Max,
        }
    }
}

/// Closed bounds used by [`super::Length::Bounded`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Bounds {
    /// A minimum pixel extent.
    Min(f32),
    /// A maximum pixel extent.
    Max(f32),
    /// Both minimum and maximum extents.
    Both {
        /// Minimum extent.
        min: f32,
        /// Maximum extent.
        max: f32,
    },
}

impl Bounds {
    /// Replaces or adds the minimum boundary.
    pub fn min(self, min: f32) -> Self {
        match self {
            Self::Min(_) => Self::Min(min),
            Self::Max(max) | Self::Both { max, .. } => Self::Both { min, max },
        }
    }

    /// Replaces or adds the maximum boundary.
    pub fn max(self, max: f32) -> Self {
        match self {
            Self::Max(_) => Self::Max(max),
            Self::Min(min) | Self::Both { min, .. } => Self::Both { min, max },
        }
    }

    /// Converts these bounds to their fluid equivalent.
    pub fn constraint(self) -> Constraint {
        match self {
            Self::Min(min) | Self::Both { min, .. } => Constraint::Min(min),
            Self::Max(_) => Constraint::Max,
        }
    }
}

/// Intrinsic growth strategy inside [`Bounds`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sizing {
    /// Resolve to intrinsic content size.
    Fit,
    /// Consume a weighted portion of remaining space.
    Fill(u16),
    /// Compress to the least possible size.
    Shrink,
}
