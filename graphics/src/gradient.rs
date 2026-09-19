//! A gradient that can be used as a fill for some geometry.
//!
//! For a gradient that you can use as a background variant for a widget, see [`Gradient`].
use crate::core::gradient::ColorStop;
use crate::core::{self, Color, Point, Rectangle};

use bytemuck::{Pod, Zeroable};
use half::f16;
mod packing;

use packing::pack_stops;

#[derive(Debug, Clone, Copy, PartialEq)]
/// A fill which linearly interpolates colors along a direction.
///
/// For a gradient which can be used as a fill for a background of a widget, see [`crate::core::Gradient`].
pub enum Gradient {
    /// A linear gradient interpolates colors along a direction from its `start` to its `end`
    /// point.
    Linear(Linear),
}

impl From<Linear> for Gradient {
    fn from(gradient: Linear) -> Self {
        Self::Linear(gradient)
    }
}

impl Gradient {
    /// Packs the [`Gradient`] for use in shader code.
    pub fn pack(&self) -> Packed {
        match self {
            Gradient::Linear(linear) => linear.pack(),
        }
    }
}

/// A linear gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Linear {
    /// The absolute starting position of the gradient.
    pub start: Point,

    /// The absolute ending position of the gradient.
    pub end: Point,

    /// [`ColorStop`]s along the linear gradient direction.
    pub stops: [Option<ColorStop>; 8],
}

impl Linear {
    /// Creates a new [`Linear`] builder.
    pub fn new(start: Point, end: Point) -> Self {
        Self {
            start,
            end,
            stops: [None; 8],
        }
    }

    /// Adds a new [`ColorStop`], defined by an offset and a color, to the gradient.
    ///
    /// Any `offset` that is not within `0.0..=1.0` will be silently ignored.
    ///
    /// Any stop added after the 8th will be silently ignored.
    pub fn add_stop(mut self, offset: f32, color: Color) -> Self {
        if offset.is_finite() && (0.0..=1.0).contains(&offset) {
            let occupied = self.stops.iter().position(Option::is_none).unwrap_or(8);

            if occupied < self.stops.len() {
                let index = self.stops[..occupied]
                    .partition_point(|stop| stop.is_some_and(|stop| stop.offset <= offset));

                self.stops.copy_within(index..occupied, index + 1);
                self.stops[index] = Some(ColorStop { offset, color });
            }
        } else {
            log::warn!("Gradient: ColorStop must be within 0.0..=1.0 range.");
        };

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

    /// Packs the [`Gradient`] for use in shader code.
    pub fn pack(&self) -> Packed {
        let (colors, offsets) = pack_stops(&self.stops);

        let direction = [self.start.x, self.start.y, self.end.x, self.end.y];

        Packed {
            colors,
            offsets,
            direction,
        }
    }
}

/// Packed [`Gradient`] data for use in shader code.
#[derive(Debug, Copy, Clone, PartialEq, Zeroable, Pod)]
#[repr(C)]
pub struct Packed {
    // 8 colors, each channel = 16 bit float, 2 colors packed into 1 u32
    colors: [[u32; 2]; 8],
    // 8 offsets, 8x 16 bit floats packed into 4 u32s
    offsets: [u32; 4],
    direction: [f32; 4],
}

impl Packed {
    /// Multiplies every packed color stop's alpha channel.
    pub fn scale_alpha(mut self, factor: f32) -> Self {
        let factor = if factor.is_finite() {
            factor.clamp(0.0, 1.0)
        } else {
            0.0
        };

        if factor >= 1.0 {
            return self;
        }

        if factor <= 0.0 {
            for color in &mut self.colors {
                color[1] &= 0xffff_0000;
            }

            return self;
        }

        for color in &mut self.colors {
            let alpha = f16::from_bits((color[1] & 0xffff) as u16);
            let scaled = f16::from_f32(alpha.to_f32() * factor);
            color[1] = (color[1] & 0xffff_0000) | u32::from(scaled.to_bits());
        }

        self
    }
}

/// Creates a new [`Packed`] gradient for use in shader code.
pub fn pack(gradient: &core::Gradient, bounds: Rectangle) -> Packed {
    match gradient {
        core::Gradient::Linear(linear) => {
            let (colors, offsets) = pack_stops(&linear.stops);

            let (start, end) = bounds.chord(linear.angle);

            let direction = [start.x, start.y, end.x, end.y];

            Packed {
                colors,
                offsets,
                direction,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stop(offset: f32) -> ColorStop {
        ColorStop {
            offset,
            color: Color::WHITE,
        }
    }

    #[test]
    fn inserting_a_middle_stop_preserves_existing_sorted_stops() {
        let gradient = Linear::new(Point::ORIGIN, Point::new(10.0, 0.0))
            .add_stop(0.2, Color::BLACK)
            .add_stop(0.8, Color::WHITE)
            .add_stop(0.5, Color::TRANSPARENT);

        assert_eq!(
            gradient.stops[..3]
                .iter()
                .map(|stop| stop.unwrap().offset)
                .collect::<Vec<_>>(),
            [0.2, 0.5, 0.8]
        );
    }

    #[test]
    fn a_ninth_stop_is_ignored_without_replacing_a_visible_stop() {
        let gradient = Linear::new(Point::ORIGIN, Point::new(10.0, 0.0))
            .add_stops((0..8).map(|index| stop(index as f32 / 7.0)));
        let unchanged = gradient.add_stop(0.5, Color::TRANSPARENT);

        assert_eq!(unchanged, gradient);
    }

    #[test]
    fn alpha_scaling_handles_identity_zero_and_invalid_factors() {
        let packed = Linear::new(Point::ORIGIN, Point::new(10.0, 0.0))
            .add_stop(0.0, Color::WHITE)
            .add_stop(1.0, Color::BLACK)
            .pack();

        assert_eq!(packed.scale_alpha(1.0), packed);
        assert_eq!(packed.scale_alpha(f32::NAN), packed.scale_alpha(0.0));

        for color in packed.scale_alpha(0.0).colors {
            assert_eq!(color[1] & 0xffff, 0);
        }
    }
}
