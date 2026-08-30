//! The colour-space contract, asserted against the build that ships.
//!
//! Taim's renderer has one documented colour contract
//! (`docs/architecture/color-pipeline.md`):
//!
//! ```text
//! authored colour  ->  decode transfer  ->  LINEAR WORKING SPACE
//!   ->  all blending, effects, compositing
//!   ->  output transform / tone map  ->  target transfer  ->  surface
//! ```
//!
//! The middle step is the one that is easy to lose and impossible to notice.
//! Blending gamma-encoded components is not a subtle inaccuracy: a 50% white
//! over black comes out at 0.5 instead of the physically correct ~0.73 in
//! sRGB-encoded output, which is a visible difference on every translucent
//! surface, every antialiased glyph edge, and every soft shadow in the
//! framework.
//!
//! These tests exist because that is exactly what happened. `web-colors` --
//! a feature whose own Cargo.toml description is "broken 'sRGB linear'
//! blending to reproduce color management of the Web" -- was a **default**
//! feature, so the shipping renderer blended in gamma space, and *no test in
//! this repository could observe it*: the root crate lists itself as a
//! dev-dependency with default features on, so even
//! `cargo test --no-default-features` had the feature enabled.
//!
//! A guard that depends on a developer remembering a flag is not a guard.

// `GAMMA_CORRECTION` is a `const bool` chosen by feature resolution, so these
// assertions are constant by construction. That is exactly what is being
// tested: the question is not what a value became at runtime, it is which
// constant the feature graph selected for the build that is about to ship.
// Clippy's `assertions_on_constants` is right in general and wrong here.
#![allow(clippy::assertions_on_constants)]

use taim_graphics::color;

/// The default build must composite in linear light.
///
/// Regression test for the defect above. This fails if `web-colors` ever
/// returns to the default feature set, or if a dev-dependency edge
/// re-enables it -- which is how it was invisible the first time.
#[test]
#[cfg(not(feature = "web-colors"))]
fn the_default_build_composites_in_linear_light() {
    assert!(
        color::GAMMA_CORRECTION,
        "this build does not composite in linear light. `web-colors` is \
         enabled somewhere in the feature graph -- most likely re-introduced \
         either into the `taim` crate's `default` list or through a \
         dev-dependency that does not set `default-features = false`. Every \
         blend in the framework is operating on gamma-encoded components."
    );
}

/// The legacy mode must still be reachable, and must still be legacy.
///
/// Compatibility behaviour is allowed to exist; it is not allowed to be the
/// definition of correct. This is the other half of the guard: it asserts
/// that asking for `web-colors` gets you the documented legacy semantics
/// rather than silently getting the correct ones.
#[test]
#[cfg(feature = "web-colors")]
fn the_legacy_web_colours_build_is_explicitly_non_linear() {
    assert!(
        !color::GAMMA_CORRECTION,
        "`web-colors` is enabled but compositing is linear, so the legacy \
         compatibility mode no longer does what it claims"
    );
}

/// Linearisation must be a real transfer function, not a pass-through.
///
/// A guard with teeth against `pack` being stubbed to the identity: sRGB
/// mid-grey is famously ~0.214 in linear light, and any implementation that
/// returns 0.5 has skipped the transfer entirely.
#[test]
#[cfg(not(feature = "web-colors"))]
fn packing_applies_the_srgb_transfer() {
    let packed = color::pack(taim_core::Color::from_rgb(0.5, 0.5, 0.5));
    let [red, ..] = packed.components();

    assert!(
        (red - 0.214_041).abs() < 1e-4,
        "sRGB 0.5 should linearise to ~0.214, got {red}"
    );
}

/// Alpha is coverage, and must survive linearisation untouched.
#[test]
fn packing_never_transfers_alpha() {
    for alpha in [0.0, 0.25, 0.5, 1.0] {
        let packed = color::pack(taim_core::Color::from_rgba8(0x80, 0x80, 0x80, alpha));

        assert!(
            (packed.components()[3] - alpha).abs() < 1e-6,
            "alpha {alpha} was transformed to {}",
            packed.components()[3]
        );
    }
}

/// Black and white are fixed points of every transfer function.
///
/// They are also why a double-encode is so hard to spot by eye: both errors
/// leave these two exactly right and move only the midtones.
#[test]
fn black_and_white_are_fixed_points() {
    let black = color::pack(taim_core::Color::BLACK).components();
    let white = color::pack(taim_core::Color::WHITE).components();

    for channel in 0..3 {
        assert!(black[channel].abs() < 1e-6, "black moved: {black:?}");
        assert!(
            (white[channel] - 1.0).abs() < 1e-6,
            "white moved: {white:?}"
        );
    }
}

/// Authored 8-bit pixel data must be decoded, not passed through.
///
/// Regression test for a defect introduced while making the CPU backend
/// colour managed: text colour, decoded image samples, colour-emoji bitmaps
/// and rasterised SVG all reach `tiny_skia` as 8-bit values that never pass
/// through `into_color`. With the frame now encoded on the way out, treating
/// those bytes as if they were already linear made every one of them present
/// far too bright — sRGB 128 text left the backend as 188.
///
/// It was invisible to the golden suite, because a text-brightness error and
/// a legitimate blend-space correction produce differences in the same range.
#[test]
#[cfg(not(feature = "web-colors"))]
fn authored_bytes_decode_into_the_working_space() {
    use taim_tiny_skia::{decode_channel, encode_channel};

    // sRGB mid-grey is ~0.214 in linear light, so 128 must become ~55.
    // Passing it through unchanged is the bug.
    let decoded = decode_channel(128);

    assert!(
        (i16::from(decoded) - 55).abs() <= 2,
        "sRGB 128 decoded to {decoded}, expected ~55 (128 means no decode at all)"
    );

    // And the pair must be inverse, or a frame drifts every time it is
    // decoded and re-encoded.
    for authored in 0..=255u8 {
        let round_tripped = encode_channel(decode_channel(authored));

        assert!(
            (i16::from(round_tripped) - i16::from(authored)).abs() <= 6,
            "{authored} round-tripped to {round_tripped}"
        );
    }
}
