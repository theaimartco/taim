//! Track mouse clicks.
use crate::mouse::Button;
use crate::time::Instant;
use crate::{Point, Transformation};

use std::ops::Mul;

/// A mouse click.
#[derive(Debug, Clone, Copy)]
pub struct Click {
    sequence: u8,
    button: Button,
    position: Point,
    time: Instant,
}

/// The kind of mouse click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A single click
    Single,

    /// A double click
    Double,

    /// A triple click
    Triple,
}

impl Click {
    /// Creates a new [`Click`] with the given position and previous last
    /// [`Click`].
    pub fn new(position: Point, button: Button, previous: Option<Click>) -> Click {
        Self::at(position, button, previous, Instant::now())
    }

    fn at(position: Point, button: Button, previous: Option<Click>, time: Instant) -> Click {
        let sequence = previous
            .filter(|click| click.button == button && click.is_consecutive(position, time))
            .map_or(1, |click| if click.sequence >= 3 { 2 } else { click.sequence + 1 });

        Click {
            sequence,
            button,
            position,
            time,
        }
    }

    /// Returns the [`Kind`] of [`Click`].
    pub fn kind(&self) -> Kind {
        match self.sequence {
            1 => Kind::Single,
            2 => Kind::Double,
            _ => Kind::Triple,
        }
    }

    /// Returns the position of the [`Click`].
    pub fn position(&self) -> Point {
        self.position
    }

    /// Returns the button that produced this click.
    pub fn button(&self) -> Button {
        self.button
    }

    fn is_consecutive(&self, new_position: Point, time: Instant) -> bool {
        let delta = new_position - self.position;
        let distance_squared = delta.x.mul_add(delta.x, delta.y * delta.y);
        let elapsed = time.checked_duration_since(self.time);

        distance_squared < 36.0
            && elapsed.is_some_and(|duration| !duration.is_zero() && duration.as_millis() <= 300)
    }
}

impl Mul<Transformation> for Click {
    type Output = Click;

    fn mul(self, transformation: Transformation) -> Click {
        Click {
            sequence: self.sequence,
            button: self.button,
            position: self.position * transformation,
            time: self.time,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn sequence_advances_without_square_root_distance() {
        let start = Instant::now();
        let first = Click::at(Point::ORIGIN, Button::Left, None, start);
        let second = Click::at(
            Point::new(3.0, 4.0),
            Button::Left,
            Some(first),
            start + Duration::from_millis(100),
        );
        let third = Click::at(
            Point::new(3.0, 4.0),
            Button::Left,
            Some(second),
            start + Duration::from_millis(200),
        );

        assert_eq!(first.kind(), Kind::Single);
        assert_eq!(second.kind(), Kind::Double);
        assert_eq!(third.kind(), Kind::Triple);
    }

    #[test]
    fn a_different_button_starts_a_new_sequence() {
        let start = Instant::now();
        let first = Click::at(Point::ORIGIN, Button::Left, None, start);
        let next = Click::at(
            Point::ORIGIN,
            Button::Right,
            Some(first),
            start + Duration::from_millis(1),
        );

        assert_eq!(next.kind(), Kind::Single);
        assert_eq!(next.button(), Button::Right);
    }
}
