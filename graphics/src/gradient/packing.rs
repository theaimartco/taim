use crate::color;
use crate::core::{Color, gradient::ColorStop};

use half::f16;

pub(super) fn pack_stops(stops: &[Option<ColorStop>; 8]) -> ([[u32; 2]; 8], [u32; 4]) {
    let mut colors = [[0; 2]; 8];
    let mut offsets = [f16::from_f32(2.0); 8];

    for (index, stop) in stops.iter().copied().enumerate() {
        let stop_color = stop.map_or_else(Color::default, |stop| stop.color);
        let [red, green, blue, alpha] = color::pack(stop_color).components();

        colors[index] = [
            pair(f16::from_f32(red), f16::from_f32(green)),
            pair(f16::from_f32(blue), f16::from_f32(alpha)),
        ];

        if let Some(stop) = stop {
            offsets[index] = f16::from_f32(stop.offset);
        }
    }

    let offsets = [
        pair(offsets[0], offsets[1]),
        pair(offsets[2], offsets[3]),
        pair(offsets[4], offsets[5]),
        pair(offsets[6], offsets[7]),
    ];

    (colors, offsets)
}

#[inline]
fn pair(high: f16, low: f16) -> u32 {
    (u32::from(high.to_bits()) << 16) | u32::from(low.to_bits())
}
