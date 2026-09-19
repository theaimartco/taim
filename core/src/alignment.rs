//! Align and position widgets.

/// Alignment on the axis of a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Alignment {
    /// Align at the start of the axis.
    #[default]
    Start,

    /// Align at the center of the axis.
    Center,

    /// Align at the end of the axis.
    End,
}

impl Alignment {
    /// Returns the normalized position along an axis.
    pub const fn factor(self) -> f32 {
        match self {
            Self::Start => 0.0,
            Self::Center => 0.5,
            Self::End => 1.0,
        }
    }

    /// Resolves the alignment inside an amount of free space.
    pub fn offset(self, free_space: f32) -> f32 {
        self.factor() * free_space
    }

    /// Mirrors start and end while preserving center.
    pub const fn mirrored(self) -> Self {
        match self {
            Self::Start => Self::End,
            Self::Center => Self::Center,
            Self::End => Self::Start,
        }
    }
}

impl From<Horizontal> for Alignment {
    fn from(horizontal: Horizontal) -> Self {
        match horizontal {
            Horizontal::Left => Self::Start,
            Horizontal::Center => Self::Center,
            Horizontal::Right => Self::End,
        }
    }
}

impl From<Vertical> for Alignment {
    fn from(vertical: Vertical) -> Self {
        match vertical {
            Vertical::Top => Self::Start,
            Vertical::Center => Self::Center,
            Vertical::Bottom => Self::End,
        }
    }
}

/// The horizontal [`Alignment`] of some resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Horizontal {
    /// Align left
    #[default]
    Left,

    /// Horizontally centered
    Center,

    /// Align right
    Right,
}

impl From<Alignment> for Horizontal {
    fn from(alignment: Alignment) -> Self {
        match alignment {
            Alignment::Start => Self::Left,
            Alignment::Center => Self::Center,
            Alignment::End => Self::Right,
        }
    }
}

/// The vertical [`Alignment`] of some resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Vertical {
    /// Align top
    #[default]
    Top,

    /// Vertically centered
    Center,

    /// Align bottom
    Bottom,
}

/// Two-axis alignment used by containers and overlays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Alignment2D {
    /// Horizontal placement.
    pub horizontal: Horizontal,
    /// Vertical placement.
    pub vertical: Vertical,
}

impl Alignment2D {
    /// Alignment centered on both axes.
    pub const CENTER: Self = Self {
        horizontal: Horizontal::Center,
        vertical: Vertical::Center,
    };

    /// Creates a two-axis alignment.
    pub const fn new(horizontal: Horizontal, vertical: Vertical) -> Self {
        Self {
            horizontal,
            vertical,
        }
    }

    /// Resolves offsets within horizontal and vertical free space.
    pub fn offsets(self, horizontal_space: f32, vertical_space: f32) -> (f32, f32) {
        (
            Alignment::from(self.horizontal).offset(horizontal_space),
            Alignment::from(self.vertical).offset(vertical_space),
        )
    }
}

impl From<Alignment> for Vertical {
    fn from(alignment: Alignment) -> Self {
        match alignment {
            Alignment::Start => Self::Top,
            Alignment::Center => Self::Center,
            Alignment::End => Self::Bottom,
        }
    }
}
