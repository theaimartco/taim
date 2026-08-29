//! The quality tier must change what the renderer actually does.
//!
//! # The defect
//!
//! `graphics::quality::Tier` defines four tiers, each with a
//! `texture_budget_bytes`. Nothing read it. `effects::pool::Pool` -- the
//! bounded accounting built to enforce it -- had no production call site, and
//! `wgpu/src/lib.rs` passed `Tier::default()` *literally* into the effect
//! pipeline, so no application could select another tier even if something
//! had been listening.
//!
//! The whole thermal- and battery-response story was therefore decorative
//! below the blur-radius gate: a device under sustained load could not make
//! the renderer allocate less.
//!
//! These tests drive the real `Renderer`, through the real effect path, and
//! assert on production telemetry rather than on internals.

use aura_core::Renderer as _;
use aura_core::{Color, Rectangle, Size};
use aura_graphics::quality::Tier;
use aura_wgpu::Renderer;
use aura_wgpu::graphics::Viewport;

const SIDE: u32 = 512;

fn renderer() -> Option<Renderer> {
    aura_futures::futures::executor::block_on(<Renderer as aura_core::renderer::Headless>::new(
        aura_core::renderer::Settings::default(),
        None,
    ))
}

macro_rules! needs_gpu {
    ($renderer:ident) => {
        #[allow(unused_mut)]
        let Some(mut $renderer) = renderer() else {
            eprintln!("skipped: no wgpu adapter available");
            return;
        };
    };
}

/// Draws a blurred square and submits the frame.
fn blurred_frame(renderer: &mut Renderer) {
    let logical = Size::new(SIDE as f32, SIDE as f32);

    renderer.reset(Rectangle::with_size(logical));

    renderer.fill_quad(
        aura_core::renderer::Quad {
            bounds: Rectangle {
                x: 64.0,
                y: 64.0,
                width: 256.0,
                height: 256.0,
            },
            ..Default::default()
        },
        Color::WHITE,
    );

    let _ = renderer.add_effect(
        aura_wgpu::effects::Effect::Blur(aura_wgpu::effects::blur::Blur { radius: 8.0 }),
        renderer.quality(),
    );

    let viewport = Viewport::with_physical_size(Size::new(SIDE, SIDE), 1.0);
    let _ = renderer.screenshot(&viewport, Color::BLACK);
}

#[test]
fn the_budget_the_renderer_uses_follows_the_tier() {
    // The direct regression test: before this, the tier was a literal at the
    // call site and every tier produced `Balanced`'s budget.
    needs_gpu!(renderer);

    for tier in [Tier::Ultra, Tier::Balanced, Tier::Efficient, Tier::Recovery] {
        renderer.set_quality(tier);
        blurred_frame(&mut renderer);

        let (_, budget) = renderer.effect_memory();

        assert_eq!(
            budget,
            tier.policy().texture_budget_bytes,
            "{tier:?} did not reach the effect pipeline"
        );
    }
}

#[test]
fn intermediates_stay_inside_the_budget() {
    // The property the pool exists for, asserted on the live path.
    needs_gpu!(renderer);

    for tier in [Tier::Ultra, Tier::Balanced, Tier::Efficient, Tier::Recovery] {
        renderer.set_quality(tier);
        blurred_frame(&mut renderer);

        let (used, budget) = renderer.effect_memory();

        assert!(
            used <= budget,
            "{tier:?}: {used} bytes accounted against a {budget} byte budget"
        );
    }
}

#[test]
fn a_smaller_budget_produces_smaller_intermediates_or_none() {
    // Budget pressure must be *measurable*, not merely recorded. A tighter
    // tier has to either shrink the intermediates or refuse them.
    needs_gpu!(renderer);

    renderer.set_quality(Tier::Ultra);
    blurred_frame(&mut renderer);
    let generous = renderer.effect_target_size();

    renderer.set_quality(Tier::Recovery);
    blurred_frame(&mut renderer);
    let constrained = renderer.effect_target_size();

    let (_, ultra_budget) = {
        renderer.set_quality(Tier::Ultra);
        (Tier::Ultra, Tier::Ultra.policy().texture_budget_bytes)
    };
    let recovery_budget = Tier::Recovery.policy().texture_budget_bytes;

    assert!(
        recovery_budget < ultra_budget,
        "the tiers under test do not actually differ in budget"
    );

    match (generous, constrained) {
        (Some(large), Some(small)) => assert!(
            small.0 * small.1 <= large.0 * large.1,
            "a tighter budget produced larger intermediates: {small:?} vs {large:?}"
        ),
        (Some(_), None) => {
            // Refused outright, which is the strongest form of the response.
        }
        (None, _) => {
            // Ultra could not admit them either; nothing to compare, and the
            // budget is still being consulted, which the other tests cover.
        }
    }
}

#[test]
fn recovery_refuses_the_blur_outright() {
    // `Tier::Recovery` sets `max_blur_radius` to 0.0, so a positive-radius
    // blur must be rejected rather than silently clamped to nothing. This
    // half of the tier gate was already live; it is pinned here so that
    // wiring the budget did not disturb it.
    needs_gpu!(renderer);

    renderer.reset(Rectangle::with_size(Size::new(64.0, 64.0)));

    assert!(
        !renderer.add_effect(
            aura_wgpu::effects::Effect::Blur(aura_wgpu::effects::blur::Blur { radius: 8.0 }),
            Tier::Recovery,
        ),
        "Recovery admitted a positive-radius blur"
    );

    assert!(
        renderer.add_effect(
            aura_wgpu::effects::Effect::Blur(aura_wgpu::effects::blur::Blur { radius: 8.0 }),
            Tier::Balanced,
        ),
        "Balanced rejected a blur well inside its radius limit"
    );
}

#[test]
fn the_default_tier_is_unchanged() {
    // Wiring the tier must not have moved the default, or every existing
    // application silently changed quality.
    needs_gpu!(renderer);

    assert_eq!(renderer.quality(), Tier::default());
    assert_eq!(Tier::default(), Tier::Balanced);
}
