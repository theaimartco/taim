//! Colors that transition progressively.
use crate::{Color, Radians};

#[derive(Debug, Clone, Copy, PartialEq)]
/// A fill which transitions colors progressively along a direction, either linearly, radially (TBD),
/// or conically (TBD).
pub enum Gradient {
    /// A linear gradient interpolates colors along a direction at a specific angle.
    Linear(Linear),
}

impl Gradient {
    /// Scales the alpha channel of the [`Gradient`] by the given factor.
    pub fn scale_alpha(self, factor: f32) -> Self {
        match self {
            Gradient::Linear(linear) => Gradient::Linear(linear.scale_alpha(factor)),
        }
    }
}

impl From<Linear> for Gradient {
    fn from(gradient: Linear) -> Self {
        Self::Linear(gradient)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
/// A point along the gradient vector where the specified [`color`] is unmixed.
///
/// [`color`]: Self::color
pub struct ColorStop {
    /// Offset along the gradient vector.
    pub offset: f32,

    /// The color of the gradient at the specified [`offset`].
    ///
    /// [`offset`]: Self::offset
    pub color: Color,
}

/// A linear gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Linear {
    /// How the [`Gradient`] is angled within its bounds.
    pub angle: Radians,
    /// [`ColorStop`]s along the linear gradient path.
    pub stops: [Option<ColorStop>; 8],
}

impl Linear {
    /// Creates a new [`Linear`] gradient with the given angle in [`Radians`].
    pub fn new(angle: impl Into<Radians>) -> Self {
        Self {
            angle: angle.into(),
            stops: [None; 8],
        }
    }

    /// Adds a new [`ColorStop`], defined by an offset and a color, to the gradient.
    ///
    /// Any `offset` that is not within `0.0..=1.0` will be silently ignored.
    ///
    /// Any stop added after the 8th will be silently ignored.
    pub fn add_stop(mut self, offset: f32, color: Color) -> Self {
        if !offset.is_finite() || !(0.0..=1.0).contains(&offset) {
            log::warn!("Gradient color stop must be within 0.0..=1.0 range.");
            return self;
        }

        let count = self.stop_count();
        if count == self.stops.len() {
            return self;
        }

        // Insert after an equal offset. Adjacent equal stops intentionally
        // describe a hard color edge and must not overwrite one another.
        let index = self.stops[..count]
            .partition_point(|stop| stop.is_some_and(|stop| stop.offset <= offset));

        self.stops.copy_within(index..count, index + 1);
        self.stops[index] = Some(ColorStop { offset, color });

        self
    }

    /// Adds multiple [`ColorStop`]s to the gradient.
    ///
    /// Any stop added after the 8th will be silently ignored.
    pub fn add_stops(mut self, stops: impl IntoIterator<Item = ColorStop>) -> Self {
        for stop in stops {
            self = self.add_stop(stop.offset, stop.color);
        }

        self
    }

    /// Scales the alpha channel of the [`Linear`] gradient by the given
    /// factor.
    pub fn scale_alpha(mut self, factor: f32) -> Self {
        for stop in self.stops.iter_mut().flatten() {
            stop.color.a *= factor;
        }

        self
    }

    /// Returns the number of populated color stops.
    #[inline]
    pub fn stop_count(&self) -> usize {
        self.stops
            .iter()
            .position(Option::is_none)
            .unwrap_or(self.stops.len())
    }
}

#[cfg(test)]
mod tests {
    use super::Linear;
    use crate::{Color, Radians};

    #[test]
    fn inserting_between_stops_preserves_existing_colors() {
        let gradient = Linear::new(Radians(0.0))
            .add_stop(0.0, Color::BLACK)
            .add_stop(1.0, Color::WHITE)
            .add_stop(0.5, Color::from_rgb(1.0, 0.0, 0.0));

        let stops: Vec<_> = gradient.stops.into_iter().flatten().collect();
        assert_eq!(stops.len(), 3);
        assert_eq!(stops[0].color, Color::BLACK);
        assert_eq!(stops[1].offset, 0.5);
        assert_eq!(stops[2].color, Color::WHITE);
    }

    #[test]
    fn equal_offsets_create_a_hard_edge_and_capacity_is_bounded() {
        let mut gradient = Linear::new(Radians(0.0));
        for index in 0..10 {
            gradient = gradient.add_stop(0.5, Color::from_rgb(index as f32 / 10.0, 0.0, 0.0));
        }

        assert_eq!(gradient.stop_count(), 8);
        assert!(gradient.stops.iter().all(Option::is_some));
    }
}
