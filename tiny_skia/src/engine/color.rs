use crate::core::Color;
use std::sync::LazyLock;

const CHANNELS: usize = 256;
const PREMULTIPLIED_TABLE_LEN: usize = CHANNELS * CHANNELS;

/// Converts an authored TAIM color to the BGRA working order used by the
/// software surface.
pub(crate) fn into_color(color: Color) -> tiny_skia::Color {
    let [red, green, blue, alpha] = working_components(color);
    tiny_skia::Color::from_rgba(blue, green, red, alpha).unwrap_or(tiny_skia::Color::TRANSPARENT)
}

pub(crate) fn working_components(color: Color) -> [f32; 4] {
    if crate::graphics::color::GAMMA_CORRECTION {
        color.into_linear()
    } else {
        [color.r, color.g, color.b, color.a]
    }
}

/// Encodes one linear working-space channel for presentation.
#[must_use]
pub fn encode_channel(linear: u8) -> u8 {
    ENCODE[linear as usize]
}

/// Decodes one authored channel to the software compositing space.
#[must_use]
pub fn decode_channel(encoded: u8) -> u8 {
    if crate::graphics::color::GAMMA_CORRECTION {
        DECODE[encoded as usize]
    } else {
        encoded
    }
}

/// Converts a premultiplied sRGB pixel without running transfer-function math
/// in the raster loop. The 64 KiB table is initialized once and turns each
/// pixel into three indexed byte reads.
#[must_use]
pub fn decode_premultiplied(red: u8, green: u8, blue: u8, alpha: u8) -> (u8, u8, u8) {
    if !crate::graphics::color::GAMMA_CORRECTION || alpha == 0 {
        return (red, green, blue);
    }

    let row = usize::from(alpha) * CHANNELS;
    (
        PREMULTIPLIED_DECODE[row + usize::from(red)],
        PREMULTIPLIED_DECODE[row + usize::from(green)],
        PREMULTIPLIED_DECODE[row + usize::from(blue)],
    )
}

static DECODE: LazyLock<[u8; CHANNELS]> =
    LazyLock::new(|| std::array::from_fn(|value| quantize(decode_transfer(value as f32 / 255.0))));

static ENCODE: LazyLock<[u8; CHANNELS]> = LazyLock::new(|| {
    std::array::from_fn(|value| {
        let linear = value as f32 / 255.0;
        let encoded = if linear <= 0.003_130_8 {
            linear * 12.92
        } else {
            1.055 * linear.powf(1.0 / 2.4) - 0.055
        };
        quantize(encoded)
    })
});

static PREMULTIPLIED_DECODE: LazyLock<[u8; PREMULTIPLIED_TABLE_LEN]> = LazyLock::new(|| {
    std::array::from_fn(|index| {
        let alpha = (index / CHANNELS) as u8;
        let component = (index % CHANNELS) as u8;
        if alpha == 0 {
            return component;
        }

        let coverage = f32::from(alpha) / 255.0;
        let straight = (f32::from(component) / 255.0 / coverage).clamp(0.0, 1.0);
        quantize(decode_transfer(straight) * coverage)
    })
});

#[inline]
fn decode_transfer(encoded: f32) -> f32 {
    if encoded <= 0.040_45 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}

#[inline]
fn quantize(channel: f32) -> u8 {
    (channel * 255.0).round().clamp(0.0, 255.0) as u8
}
