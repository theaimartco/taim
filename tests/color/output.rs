//! What the presentation target actually receives.
//!
//! This repository has repeatedly validated intermediate numbers and missed
//! wrong final output. Four defects found in this pipeline were:
//!
//! | Defect | Visible as | Caught by a numeric test? |
//! |---|---|---|
//! | Source peak defaulted to the PQ container | SDR UI ~22% too dim | no |
//! | Encoding relative to peak, not reference white | macOS EDR at ~12.7% | no |
//! | HLG decode outside its domain | infinity | no |
//! | Frame encoded by shader *and* hardware | midtones washed out | no |
//!
//! Three of the four leave black and white exactly right and move only what
//! is in between, which is why they survive every test that checks a corner
//! value or an internal quantity.
//!
//! So these tests ask one question: **given this input, what value lands in
//! the buffer that gets presented?** They compare against analytically derived
//! answers rather than against a previously accepted picture, so unlike a
//! golden they cannot ratify a bug.
//!
//! Run on the CPU backend, which needs no adapter and is therefore never
//! skipped. `tests/gpu/display_transform.rs` asks the same question of the GPU
//! path.

use taim_core::renderer::Quad;
use taim_core::{Color, Rectangle, Renderer as _, Size};
use taim_tiny_skia::Renderer;

const SIZE: u32 = 32;

/// Renders one scene and returns the presented RGBA bytes.
fn present(background: Color, draw: impl FnOnce(&mut Renderer)) -> Vec<u8> {
    let mut renderer = Renderer::new(taim_core::renderer::Settings::default());

    let bounds = Rectangle::with_size(Size::new(SIZE as f32, SIZE as f32));
    taim_core::Renderer::reset(&mut renderer, bounds);

    draw(&mut renderer);

    taim_core::renderer::Headless::screenshot(&mut renderer, Size::new(SIZE, SIZE), 1.0, background)
}

/// The channel values at the centre of the frame.
fn centre(pixels: &[u8]) -> [u8; 4] {
    let index = ((SIZE / 2 * SIZE + SIZE / 2) * 4) as usize;

    [
        pixels[index],
        pixels[index + 1],
        pixels[index + 2],
        pixels[index + 3],
    ]
}

/// Fills the whole frame with `color` at `alpha`.
fn wash(color: Color, alpha: f32) -> Vec<u8> {
    present(Color::BLACK, |renderer| {
        renderer.fill_quad(
            Quad {
                bounds: Rectangle::with_size(Size::new(SIZE as f32, SIZE as f32)),
                ..Default::default()
            },
            Color { a: alpha, ..color },
        );
    })
}

/// sRGB encode, for deriving what a linear blend must produce.
///
/// Only the linear-contract tests need it, and those are compiled out under
/// the `web-colors` compatibility feature.
#[cfg(not(feature = "web-colors"))]
fn encode(linear: f32) -> f32 {
    if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}

#[test]
fn an_opaque_fill_reaches_the_target_unchanged() {
    // The round trip through the working space must be identity for opaque
    // content. If decode and encode disagree -- one applied twice, or one
    // missing -- this is where it shows first.
    for authored in [0u8, 16, 64, 128, 200, 255] {
        let value = f32::from(authored) / 255.0;
        let pixels = wash(Color::from_rgb(value, value, value), 1.0);
        let [red, ..] = centre(&pixels);

        assert!(
            red.abs_diff(authored) <= 6,
            "authored {authored} was presented as {red}"
        );
    }
}

// Asserts the production colour contract, so it is meaningful only where that
// contract holds. Under the `web-colors` compatibility feature blending is
// gamma-space by design and this test correctly fails; gating keeps the legacy
// configuration testable instead of permanently red.
#[cfg(not(feature = "web-colors"))]
#[test]
fn a_half_covered_white_blends_in_linear_light() {
    // The single most diagnostic value in the whole pipeline.
    //
    //   gamma-space blend of white over black at alpha 0.5 -> 128
    //   linear-space blend                                 -> 188
    //
    // A 60/255 difference, and the entire reason `web-colors` was removed
    // from the default feature set.
    let pixels = wash(Color::WHITE, 0.5);
    let [red, ..] = centre(&pixels);

    let expected = (encode(0.5) * 255.0).round() as u8;

    assert!(
        red.abs_diff(expected) <= 6,
        "a 50% white wash presented as {red}; linear light requires {expected} \
         (gamma-space blending would give 128)"
    );
}

// Asserts the production colour contract, so it is meaningful only where that
// contract holds. Under the `web-colors` compatibility feature blending is
// gamma-space by design and this test correctly fails; gating keeps the legacy
// configuration testable instead of permanently red.
#[cfg(not(feature = "web-colors"))]
#[test]
fn partial_coverage_follows_the_linear_curve_across_its_range() {
    // Not just one point: the whole curve. A pipeline that happened to be
    // right at 0.5 and wrong elsewhere would pass the test above.
    for alpha in [0.25_f32, 0.5, 0.75] {
        let pixels = wash(Color::WHITE, alpha);
        let [red, ..] = centre(&pixels);

        let expected = (encode(alpha) * 255.0).round() as u8;

        assert!(
            red.abs_diff(expected) <= 8,
            "alpha {alpha} presented as {red}, linear light requires {expected}"
        );
    }
}

#[test]
fn a_midtone_is_not_encoded_twice() {
    // A double encode leaves black and white exactly right and pushes
    // midtones up: sRGB 0.5 becomes 0.74, i.e. 128 -> 188. Indistinguishable
    // from a badly tuned theme unless something checks the number.
    let pixels = wash(Color::from_rgb(0.5, 0.5, 0.5), 1.0);
    let [red, ..] = centre(&pixels);

    assert!(
        red.abs_diff(128) <= 6,
        "opaque mid-grey presented as {red}; 188 would mean a second encode, \
         55 would mean a second decode"
    );
}

// Asserts the production colour contract, so it is meaningful only where that
// contract holds. Under the `web-colors` compatibility feature blending is
// gamma-space by design and this test correctly fails; gating keeps the legacy
// configuration testable instead of permanently red.
#[cfg(not(feature = "web-colors"))]
#[test]
fn stacked_translucent_layers_accumulate_correctly() {
    // Two 50% white layers over black. In linear light the coverage composes
    // as 1 - 0.5^2 = 0.75, so the presented value is encode(0.75), not
    // encode(0.5) applied twice and not 0.75 in encoded units.
    let pixels = present(Color::BLACK, |renderer| {
        for _ in 0..2 {
            renderer.fill_quad(
                Quad {
                    bounds: Rectangle::with_size(Size::new(SIZE as f32, SIZE as f32)),
                    ..Default::default()
                },
                Color {
                    a: 0.5,
                    ..Color::WHITE
                },
            );
        }
    });

    let [red, ..] = centre(&pixels);
    let expected = (encode(0.75) * 255.0).round() as u8;

    assert!(
        red.abs_diff(expected) <= 8,
        "two 50% layers presented as {red}, linear light requires {expected}"
    );
}

#[test]
fn a_saturated_colour_keeps_its_hue_through_the_pipeline() {
    // A conversion applied to the wrong channels, or a channel swap, shows up
    // as a hue change rather than a brightness change, and no luminance test
    // detects it.
    let pixels = wash(Color::from_rgb8(0xFF, 0x00, 0x00), 1.0);
    let [red, green, blue, _] = centre(&pixels);

    assert!(red > 240, "red channel came through as {red}");
    assert!(green < 12, "green channel should be empty, got {green}");
    assert!(blue < 12, "blue channel should be empty, got {blue}");
}

#[test]
fn the_background_reaches_the_target() {
    // The clear colour goes through a different path from a drawn quad, and
    // has its own opportunity to miss a transfer.
    for authored in [0u8, 64, 128, 255] {
        let value = f32::from(authored) / 255.0;
        let pixels = present(Color::from_rgb(value, value, value), |_| {});
        let [red, ..] = centre(&pixels);

        assert!(
            red.abs_diff(authored) <= 6,
            "background {authored} presented as {red}"
        );
    }
}

#[test]
fn alpha_reaches_the_target_untouched() {
    // Coverage is not light and must never be transferred.
    let pixels = wash(Color::WHITE, 1.0);
    let [.., alpha] = centre(&pixels);

    assert_eq!(alpha, 255, "an opaque frame presented alpha {alpha}");
}

#[test]
fn no_presented_pixel_is_ever_out_of_range() {
    // `u8` cannot hold a NaN, so a NaN upstream arrives as an arbitrary
    // value. What this can prove is that nothing clipped to an extreme where
    // the scene had a midtone -- the signature of an infinity reaching the
    // encoder.
    let pixels = present(Color::from_rgb(0.25, 0.5, 0.75), |renderer| {
        renderer.fill_quad(
            Quad {
                bounds: Rectangle {
                    x: 4.0,
                    y: 4.0,
                    width: 24.0,
                    height: 24.0,
                },
                ..Default::default()
            },
            Color {
                a: 0.5,
                ..Color::from_rgb8(0x20, 0xC0, 0x60)
            },
        );
    });

    let [red, green, blue, _] = centre(&pixels);

    for (name, channel) in [("red", red), ("green", green), ("blue", blue)] {
        assert!(
            channel > 0 && channel < 255,
            "{name} clipped to {channel}; a midtone scene should not reach an extreme"
        );
    }
}
