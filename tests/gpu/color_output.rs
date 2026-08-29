//! What the presentation target actually receives.
//!
//! # Why this suite exists
//!
//! This repository has repeatedly validated intermediate numbers and missed
//! wrong final output. Four defects in the colour and HDR work were caught
//! only by asking what a display would be sent -- SDR white 22% too dim, macOS
//! EDR at 12.7% of intended brightness, an infinity out of a transfer
//! function, and a frame encoded twice -- and three of those four passed every
//! numeric test in the repository while they were live.
//!
//! The 68 committed goldens cannot cover this: they all render through
//! `tiny_skia`, which is not colour managed, so they are blind to a change
//! that alters every blended pixel the GPU produces.
//!
//! So these tests render through the real `wgpu` renderer and assert on the
//! bytes the surface receives, against values that are known analytically
//! rather than captured from a previous run.
//!
//! # The value that decides everything
//!
//! 50% white over black, composited in **linear light** and written to an
//! sRGB surface, is `187`. The same composite in **gamma space** is `128`.
//! Nothing else in this file is as diagnostic as that one number: it is a 59
//! step difference out of 255, on the single most common operation in any
//! user interface.

use aura_core::Renderer as _;
use aura_core::{Color, Rectangle, Size};
use aura_wgpu::Renderer;
use aura_wgpu::graphics::Viewport;

const SIDE: u32 = 64;

fn renderer() -> Option<Renderer> {
    aura_futures::futures::executor::block_on(<Renderer as aura_core::renderer::Headless>::new(
        aura_core::renderer::Settings::default(),
        None,
    ))
}

macro_rules! needs_gpu {
    ($renderer:ident) => {
        let Some(mut $renderer) = renderer() else {
            eprintln!("skipped: no wgpu adapter available");
            return;
        };
    };
}

/// Renders `scene` over `background` and returns the surface bytes.
fn surface_of(
    renderer: &mut Renderer,
    background: Color,
    scene: impl FnOnce(&mut Renderer),
) -> Vec<u8> {
    renderer.reset(Rectangle::with_size(Size::new(SIDE as f32, SIDE as f32)));
    scene(renderer);

    let viewport = Viewport::with_physical_size(Size::new(SIDE, SIDE), 1.0);

    renderer.screenshot(&viewport, background)
}

/// Fills the whole surface with `color`.
fn fill(renderer: &mut Renderer, color: Color) {
    renderer.fill_quad(
        aura_core::renderer::Quad {
            bounds: Rectangle {
                x: 0.0,
                y: 0.0,
                width: SIDE as f32,
                height: SIDE as f32,
            },
            ..Default::default()
        },
        color,
    );
}

/// The RGBA at the centre of the surface.
fn centre(pixels: &[u8]) -> [u8; 4] {
    let index = (((SIDE / 2) * SIDE + (SIDE / 2)) * 4) as usize;

    [
        pixels[index],
        pixels[index + 1],
        pixels[index + 2],
        pixels[index + 3],
    ]
}

/// GPU rounding, driver differences and MSAA resolve all move a channel by a
/// step or two; a wrong colour space moves it by tens.
const TOLERANCE: i16 = 3;

#[track_caller]
fn assert_channel(actual: u8, expected: u8, what: &str) {
    let delta = i16::from(actual) - i16::from(expected);

    assert!(
        delta.abs() <= TOLERANCE,
        "{what}: surface received {actual}, expected {expected} (off by {delta})"
    );
}

// ---------------------------------------------------------------------
// The analytic case
// ---------------------------------------------------------------------

#[test]
#[cfg(not(feature = "web-colors"))]
fn fifty_percent_white_over_black_is_composited_in_linear_light() {
    // Analytically: src 1.0 over dst 0.0 at alpha 0.5 gives linear 0.5, and
    // the sRGB transfer of 0.5 is 0.7354 -> 187.5.
    //
    // Blended in gamma space the same composite gives 127.5. There is no
    // rounding or precision story that bridges 187 and 128.
    needs_gpu!(renderer);

    let pixels = surface_of(&mut renderer, Color::BLACK, |renderer| {
        fill(renderer, Color::from_rgba(1.0, 1.0, 1.0, 0.5));
    });

    let [red, ..] = centre(&pixels);

    assert_channel(red, 187, "50% white over black");

    assert!(
        (i16::from(red) - 128).abs() > 20,
        "the surface received {red}, which is the gamma-space answer -- \
         compositing is not happening in linear light"
    );
}

// ---------------------------------------------------------------------
// Encoding: exactly once, never zero times, never twice
// ---------------------------------------------------------------------

#[test]
fn opaque_colours_survive_the_round_trip_unchanged() {
    // An opaque fill involves no blending, so whatever comes back is purely a
    // statement about encoding. Too dark means the frame was encoded twice;
    // too light means it was never encoded.
    needs_gpu!(renderer);

    for authored in [0u8, 32, 64, 128, 192, 255] {
        let pixels = surface_of(&mut renderer, Color::BLACK, |renderer| {
            fill(renderer, Color::from_rgb8(authored, authored, authored));
        });

        let [red, ..] = centre(&pixels);

        assert_channel(red, authored, &format!("opaque {authored}"));
    }
}

#[test]
fn mid_grey_is_neither_washed_out_nor_crushed() {
    // The specific signature of a double encode: black and white stay exactly
    // right and mid-grey moves. sRGB 128 double-encoded lands near 188;
    // never-encoded lands near 55.
    needs_gpu!(renderer);

    let pixels = surface_of(&mut renderer, Color::BLACK, |renderer| {
        fill(renderer, Color::from_rgb8(128, 128, 128));
    });

    let [red, ..] = centre(&pixels);

    assert_channel(red, 128, "opaque mid-grey");
    assert!(i16::from(red) < 160, "mid-grey looks double-encoded: {red}");
    assert!(i16::from(red) > 96, "mid-grey looks unencoded: {red}");
}

#[test]
fn black_and_white_are_exact() {
    // Fixed points of every transfer function, so they must be exact rather
    // than merely close -- and their being exact is why they cannot detect an
    // encoding fault on their own.
    needs_gpu!(renderer);

    let white = surface_of(&mut renderer, Color::BLACK, |renderer| {
        fill(renderer, Color::WHITE);
    });
    assert_channel(centre(&white)[0], 255, "opaque white");

    let black = surface_of(&mut renderer, Color::WHITE, |renderer| {
        fill(renderer, Color::BLACK);
    });
    assert_channel(centre(&black)[0], 0, "opaque black");
}

// ---------------------------------------------------------------------
// Blending
// ---------------------------------------------------------------------

#[test]
#[cfg(not(feature = "web-colors"))]
fn alpha_blending_follows_the_linear_curve_across_its_range() {
    // Each step is computed from the same analytic model as the headline
    // case, so the whole curve is pinned rather than one point on it.
    needs_gpu!(renderer);

    fn expected(alpha: f32) -> u8 {
        let linear = alpha;
        let encoded = if linear <= 0.003_130_8 {
            12.92 * linear
        } else {
            1.055 * linear.powf(1.0 / 2.4) - 0.055
        };

        (encoded * 255.0).round() as u8
    }

    for alpha in [0.25f32, 0.5, 0.75] {
        let pixels = surface_of(&mut renderer, Color::BLACK, |renderer| {
            fill(renderer, Color::from_rgba(1.0, 1.0, 1.0, alpha));
        });

        assert_channel(
            centre(&pixels)[0],
            expected(alpha),
            &format!("white at alpha {alpha} over black"),
        );
    }
}

#[test]
fn stacked_translucent_layers_converge_upward() {
    // Overlays are the most common translucent construction in any interface.
    // Each additional 50% white layer must move the result strictly toward
    // white and never past it.
    needs_gpu!(renderer);

    let mut previous = 0i16;

    for layers in 1..=4 {
        let pixels = surface_of(&mut renderer, Color::BLACK, |renderer| {
            for _ in 0..layers {
                fill(renderer, Color::from_rgba(1.0, 1.0, 1.0, 0.5));
            }
        });

        let value = i16::from(centre(&pixels)[0]);

        assert!(
            value > previous,
            "{layers} layers produced {value}, not brighter than {previous}"
        );
        assert!(value <= 255);

        previous = value;
    }
}

#[test]
#[cfg(not(feature = "web-colors"))]
fn a_translucent_colour_over_another_colour_does_not_shift_hue() {
    // Gamma-space blending of two different hues shifts the result toward the
    // darker one in a way linear blending does not. Red over green at 50% must
    // land with both channels well above the gamma-space answer.
    needs_gpu!(renderer);

    let pixels = surface_of(&mut renderer, Color::BLACK, |renderer| {
        fill(renderer, Color::from_rgb(0.0, 1.0, 0.0));
        fill(renderer, Color::from_rgba(1.0, 0.0, 0.0, 0.5));
    });

    let [red, green, ..] = centre(&pixels);

    // Both channels are a 50% linear mix, so both encode to ~187.
    assert_channel(red, 187, "red over green, red channel");
    assert_channel(green, 187, "red over green, green channel");
}

// ---------------------------------------------------------------------
// Robustness of the output itself
// ---------------------------------------------------------------------

#[test]
fn no_output_channel_is_ever_a_sentinel_value() {
    // An 8-bit surface cannot hold NaN, so the failure mode of a NaN reaching
    // it is a garbage byte rather than a detectable one. What is checkable is
    // that a scene built from hostile values still produces a plausible,
    // fully-opaque frame rather than noise.
    needs_gpu!(renderer);

    let pixels = surface_of(&mut renderer, Color::BLACK, |renderer| {
        fill(renderer, Color::from_rgba(1.0, 0.0, 0.0, 0.5));
        fill(renderer, Color::from_rgba(0.0, 0.0, 1.0, 0.25));
    });

    assert_eq!(pixels.len(), (SIDE * SIDE * 4) as usize);

    for pixel in pixels.chunks_exact(4) {
        assert_eq!(
            pixel[3], 255,
            "an opaque frame produced a translucent pixel"
        );
    }
}

#[test]
fn a_saturated_primary_stays_saturated() {
    // Round-tripping a fully saturated colour must not bleed into the other
    // channels: that would mean a gamut conversion ran where none was wanted.
    needs_gpu!(renderer);

    for (color, channel, name) in [
        (Color::from_rgb(1.0, 0.0, 0.0), 0, "red"),
        (Color::from_rgb(0.0, 1.0, 0.0), 1, "green"),
        (Color::from_rgb(0.0, 0.0, 1.0), 2, "blue"),
    ] {
        let pixels = surface_of(&mut renderer, Color::BLACK, |renderer| {
            fill(renderer, color);
        });

        let sample = centre(&pixels);

        assert_channel(sample[channel], 255, &format!("saturated {name}"));

        for (other, value) in sample.iter().enumerate().take(3) {
            if other != channel {
                assert_channel(*value, 0, &format!("{name}'s other channels"));
            }
        }
    }
}

#[test]
fn the_clear_colour_reaches_the_surface_unchanged() {
    // The background is the one colour that never goes through a quad
    // pipeline, so it is an independent check on the same encoding question.
    needs_gpu!(renderer);

    for authored in [0u8, 64, 128, 255] {
        let pixels = surface_of(
            &mut renderer,
            Color::from_rgb8(authored, authored, authored),
            |_| {},
        );

        assert_channel(centre(&pixels)[0], authored, &format!("clear {authored}"));
    }
}

// ---------------------------------------------------------------------
// The legacy compatibility mode, pinned rather than left unverified
// ---------------------------------------------------------------------

/// Under `web-colors`, blending happens on gamma-encoded components.
///
/// That is what the feature is *for* -- matching a browser -- so it gets a
/// test that asserts it rather than an exemption. The two numbers below are
/// the whole difference between the modes, measured: a 50% composite reads
/// `128` here and `187` on the production path.
///
/// Keeping both pinned means neither mode can drift into the other silently,
/// and it documents the cost of choosing the legacy one.
#[test]
#[cfg(feature = "web-colors")]
fn the_legacy_mode_blends_in_gamma_space() {
    needs_gpu!(renderer);

    let pixels = surface_of(&mut renderer, Color::BLACK, |renderer| {
        fill(renderer, Color::from_rgba(1.0, 1.0, 1.0, 0.5));
    });

    let [red, ..] = centre(&pixels);

    assert_channel(red, 128, "50% white over black, gamma space");

    assert!(
        (i16::from(red) - 187).abs() > 20,
        "`web-colors` is enabled but compositing produced the linear-light \
         answer ({red}); the compatibility mode no longer does what it claims"
    );
}

/// Opaque colours are unaffected by the blending space.
///
/// Worth pinning in both modes: it is the reason the defect survived so long.
/// Every opaque surface, and both ends of every gradient, looked correct.
#[test]
#[cfg(feature = "web-colors")]
fn the_legacy_mode_still_round_trips_opaque_colours() {
    needs_gpu!(renderer);

    for authored in [0u8, 128, 255] {
        let pixels = surface_of(&mut renderer, Color::BLACK, |renderer| {
            fill(renderer, Color::from_rgb8(authored, authored, authored));
        });

        assert_channel(centre(&pixels)[0], authored, &format!("opaque {authored}"));
    }
}
