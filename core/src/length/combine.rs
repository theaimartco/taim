use super::{Constraint, Length, Sizing};

impl Length {
    /// Combines main-axis constraints, adding fluid minimums.
    pub fn stack(self, other: Length) -> Self {
        self.merge_with(other, Constraint::stack)
    }

    /// Combines cross-axis constraints, retaining the largest fluid minimum.
    pub fn cross(self, other: Length) -> Self {
        self.merge_with(other, Constraint::cross)
    }

    fn merge_with(self, other: Self, merge: impl Fn(Constraint, Constraint) -> Constraint) -> Self {
        match (self, other) {
            (Self::Shrink | Self::Fixed(_) | Self::Fill | Self::FillPortion(_), _) => self,
            (Self::Fluid(a), Self::Fluid(b)) => Self::Fluid(merge(a, b)),
            (
                Self::Fluid(a),
                Self::Bounded {
                    bounds,
                    sizing: Sizing::Fill(_),
                },
            ) => Self::Fluid(merge(a, bounds.constraint())),
            (Self::Fluid(constraint), _) => Self::Fluid(constraint),
            (
                Self::Bounded {
                    bounds,
                    sizing: Sizing::Fit,
                },
                Self::Fill
                | Self::FillPortion(_)
                | Self::Bounded {
                    sizing: Sizing::Fill(_),
                    ..
                },
            ) => Self::Bounded {
                bounds,
                sizing: Sizing::Fill(1),
            },
            (Self::Bounded { bounds, sizing }, _) => Self::Bounded { bounds, sizing },
            (
                _,
                Self::Bounded {
                    bounds,
                    sizing: Sizing::Fill(_),
                },
            ) => Self::Fluid(bounds.constraint()),
            (_, Self::Fluid(constraint)) => Self::Fluid(constraint),
            (_, Self::Fill | Self::FillPortion(_)) => Self::Fill,
            _ => Self::Fit,
        }
    }
}
