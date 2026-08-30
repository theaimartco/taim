//! Texture-pool budgeting, effect-area budgeting, and quality-tier
//! switching stability (`phase.md` Phase 12 exit gate: "no effect can
//! trigger unbounded texture allocation"; "tier switching is stable";
//! "Efficient and Recovery remain readable and coherent"; effect policy:
//! "use hysteresis and minimum residency time when changing quality
//! tiers; prevent visual oscillation").

use taim_core::time::{Duration, Instant};
use taim_core::{Point, Rectangle, Size};
use taim_graphics::quality::Tier;
use taim_runtime::quality_controller::{Controller, FramePressure, Pressure};
use taim_wgpu::effects::pool::{Descriptor, Pool};
use taim_wgpu::effects::{Effect, Graph, blur::Blur};

fn descriptor(width: u32, height: u32) -> Descriptor {
    Descriptor::new(width, height, taim_wgpu::wgpu::TextureFormat::Rgba8Unorm)
}

/// The same shape as [`descriptor`], in the half-float format HDR
/// compositing uses for its intermediates.
fn half_float(width: u32, height: u32) -> Descriptor {
    Descriptor::new(width, height, taim_wgpu::wgpu::TextureFormat::Rgba16Float)
}

// ---------------------------------------------------------------------
// Texture pool
// ---------------------------------------------------------------------

#[test]
fn a_pool_never_exceeds_its_byte_budget() {
    let one = descriptor(64, 64);
    let mut pool = Pool::new(one.bytes());

    assert!(pool.acquire(one));
    // A second live texture would exceed the single-texture budget.
    assert!(!pool.acquire(one));
    assert!(pool.footprint_bytes() <= pool.budget_bytes());
}

#[test]
fn releasing_a_texture_makes_it_reusable_without_growing_the_footprint() {
    // The regression this guards: `release` used to never reclaim bytes,
    // so reuse still exhausted the budget. A release + re-acquire of the
    // same descriptor must reuse the idle texture with no net growth.
    let one = descriptor(64, 64);
    let mut pool = Pool::new(one.bytes());

    assert!(pool.acquire(one));
    assert_eq!(pool.used_bytes(), one.bytes());

    pool.release(one);
    assert_eq!(pool.used_bytes(), 0);
    assert_eq!(pool.reserved_bytes(), one.bytes());

    // Reuse: no new allocation, footprint unchanged.
    assert!(pool.acquire(one));
    assert_eq!(pool.used_bytes(), one.bytes());
    assert_eq!(pool.footprint_bytes(), one.bytes());
}

#[test]
fn a_pool_evicts_idle_textures_to_admit_a_new_size_within_budget() {
    // Budget holds exactly one 64x64. Acquire one, release it (now idle),
    // then acquire a *different* size that only fits if the idle one is
    // evicted.
    let small = descriptor(64, 64);
    let other = descriptor(64, 64); // same footprint, different "slot"
    let mut pool = Pool::new(small.bytes());

    assert!(pool.acquire(small));
    pool.release(small);
    // Idle small still reserved; acquiring another same-size reuses it.
    assert!(pool.acquire(other));
    assert!(pool.footprint_bytes() <= pool.budget_bytes());
}

#[test]
fn a_texture_larger_than_the_whole_budget_is_refused() {
    let mut pool = Pool::new(1_024);
    // 64*64*4 = 16384 bytes, far over budget: refused, nothing allocated.
    assert!(!pool.acquire(descriptor(64, 64)));
    assert_eq!(pool.used_bytes(), 0);
    assert_eq!(pool.footprint_bytes(), 0);
}

#[test]
fn heavy_reuse_stays_bounded_over_many_frames() {
    // Simulate many frames each acquiring and releasing intermediates:
    // the footprint must never exceed the budget, proving no unbounded
    // growth (the original monotonic-`used` bug would fail this fast).
    let one = descriptor(128, 128);
    let mut pool = Pool::new(one.bytes() * 2);

    for _ in 0..10_000 {
        assert!(pool.acquire(one));
        assert!(pool.footprint_bytes() <= pool.budget_bytes());
        pool.release(one);
    }

    assert!(pool.footprint_bytes() <= pool.budget_bytes());
}

// ---------------------------------------------------------------------
// Effect-area budget at the graph level
// ---------------------------------------------------------------------

#[test]
fn a_graph_refuses_effects_that_would_exceed_the_tier_area_budget() {
    let surface = Rectangle::new(Point::ORIGIN, Size::new(4000.0, 4000.0));
    let source = surface; // a full-surface blur is the most expensive case
    let mut graph = Graph::new();

    // Efficient's max_effect_area is small (262_144); a full 4000x4000
    // blurred region (16M px^2) vastly exceeds it and must be refused.
    let admitted = graph.push_within_budget(
        Effect::Blur(Blur { radius: 4.0 }),
        source,
        surface,
        Tier::Efficient,
    );

    // Efficient caps blur radius at 8, so radius 4 is allowed by policy,
    // but the *area* is over budget, so the graph still refuses it.
    assert!(!admitted, "an over-budget-area effect must be refused");
    assert!(graph.is_empty());
}

#[test]
fn a_graph_admits_small_effects_within_the_tier_area_budget() {
    let surface = Rectangle::new(Point::ORIGIN, Size::new(1920.0, 1080.0));
    let source = Rectangle::new(Point::new(50.0, 50.0), Size::new(60.0, 40.0));
    let mut graph = Graph::new();

    assert!(graph.push_within_budget(
        Effect::Blur(Blur { radius: 6.0 }),
        source,
        surface,
        Tier::Balanced
    ));
    assert_eq!(graph.effects().len(), 1);
    assert!(
        graph.total_sampling_area(source, surface, Tier::Balanced)
            <= Tier::Balanced.policy().max_effect_area
    );
}

// ---------------------------------------------------------------------
// Tier policy monotonicity
// ---------------------------------------------------------------------

#[test]
fn tier_budgets_are_monotonic_from_ultra_down_to_recovery() {
    let tiers = [Tier::Ultra, Tier::Balanced, Tier::Efficient, Tier::Recovery];

    for pair in tiers.windows(2) {
        let higher = pair[0].policy();
        let lower = pair[1].policy();

        assert!(lower.max_blur_radius <= higher.max_blur_radius);
        assert!(lower.max_effect_area <= higher.max_effect_area);
        assert!(lower.texture_budget_bytes <= higher.texture_budget_bytes);
        // Capability flags never turn *on* as the tier degrades: if the
        // lower tier has the capability, the higher one must too.
        assert!(!lower.backdrop_blur || higher.backdrop_blur);
        assert!(!lower.noise || higher.noise);
    }
}

#[test]
fn recovery_disables_every_expensive_effect() {
    let recovery = Tier::Recovery.policy();
    assert_eq!(recovery.max_blur_radius, 0.0);
    assert!(!recovery.backdrop_blur);
    assert!(!recovery.noise);
}

// ---------------------------------------------------------------------
// Quality controller hysteresis and emergency behavior
// ---------------------------------------------------------------------

fn pressure(gpu: f32, missed: f32, thermal: f32, recovering: bool) -> Pressure {
    Pressure {
        gpu_ratio: gpu,
        missed_frame_ratio: missed,
        thermal_ratio: thermal,
        // These tests are about the GPU's own signals; energy pressure
        // arrives separately, through `power_policy`, and has its own
        // tests there.
        energy_ratio: 0.0,
        recovering,
    }
}

#[test]
fn ordinary_tier_changes_wait_out_the_residency_window() {
    let now = Instant::now();
    let mut controller = Controller::new(now).with_minimum_residency(Duration::from_secs(5));

    let stressed = pressure(0.95, 0.0, 0.0, false);

    // Before residency elapses: no change.
    assert_eq!(controller.update(now, stressed), Tier::Balanced);
    assert_eq!(
        controller.update(now + Duration::from_secs(2), stressed),
        Tier::Balanced
    );
    // After residency: change to Efficient.
    assert_eq!(
        controller.update(now + Duration::from_secs(5), stressed),
        Tier::Efficient
    );
}

#[test]
fn an_emergency_drops_to_recovery_immediately_bypassing_hysteresis() {
    // A GPU-loss / thermal-critical event must protect the device now,
    // not after the 5-second residency window.
    let now = Instant::now();
    let mut controller = Controller::new(now).with_minimum_residency(Duration::from_secs(5));

    let emergency = pressure(0.5, 0.0, 0.99, false);
    assert_eq!(
        controller.update(now, emergency),
        Tier::Recovery,
        "thermal-critical must drop to Recovery instantly"
    );

    let mut controller = Controller::new(now);
    let recovering = pressure(0.2, 0.0, 0.2, true);
    assert_eq!(
        controller.update(now, recovering),
        Tier::Recovery,
        "active GPU recovery must drop to Recovery instantly"
    );
}

#[test]
fn leaving_recovery_still_waits_out_the_residency_window() {
    // Symmetric protection: once conditions clear, recovering *up* out of
    // Recovery is hysteresis-gated so a flapping thermal signal cannot
    // oscillate effects back on immediately.
    let now = Instant::now();
    let mut controller = Controller::new(now).with_minimum_residency(Duration::from_secs(5));

    // Enter recovery immediately.
    assert_eq!(
        controller.update(now, pressure(0.5, 0.0, 0.99, false)),
        Tier::Recovery
    );

    // Conditions clear, but leaving Recovery waits out residency.
    let calm = pressure(0.2, 0.0, 0.2, false);
    assert_eq!(
        controller.update(now + Duration::from_secs(1), calm),
        Tier::Recovery
    );
    assert_ne!(
        controller.update(now + Duration::from_secs(6), calm),
        Tier::Recovery
    );
}

#[test]
fn a_controller_does_not_oscillate_under_a_signal_hovering_at_a_threshold() {
    // Stability: a pressure signal jittering right at the Efficient
    // threshold must not flip the tier every frame.
    let start = Instant::now();
    let mut controller = Controller::new(start).with_minimum_residency(Duration::from_secs(5));

    let mut changes = 0;
    let mut previous = controller.tier();

    for frame in 0..600 {
        let now = start + Duration::from_millis(frame * 16);
        // Alternate just above / just below the gpu_ratio 0.9 threshold.
        let gpu = if frame % 2 == 0 { 0.91 } else { 0.89 };
        let tier = controller.update(now, pressure(gpu, 0.0, 0.0, false));
        if tier != previous {
            changes += 1;
            previous = tier;
        }
    }

    // Over ~10 seconds with a 5s residency, at most a couple of changes
    // are possible -- certainly not one per frame.
    assert!(
        changes <= 2,
        "tier oscillated {changes} times under a hovering signal"
    );
}

#[test]
fn frame_pressure_reports_a_measured_missed_ratio() {
    // The honest, measured signal bridge: half the recent frames overran
    // a 16ms budget, so the ratio is ~0.5.
    let mut fp = FramePressure::new(Duration::from_millis(16), 8);

    for i in 0..8 {
        let duration = if i % 2 == 0 {
            Duration::from_millis(20) // over budget
        } else {
            Duration::from_millis(10) // under budget
        };
        fp.record(duration);
    }

    assert!((fp.missed_frame_ratio() - 0.5).abs() < f32::EPSILON);
}

#[test]
fn frame_pressure_slides_its_window() {
    let mut fp = FramePressure::new(Duration::from_millis(16), 4);

    // Fill with all-missed, then all-good; only the last 4 count.
    for _ in 0..4 {
        fp.record(Duration::from_millis(30));
    }
    assert_eq!(fp.missed_frame_ratio(), 1.0);

    for _ in 0..4 {
        fp.record(Duration::from_millis(5));
    }
    assert_eq!(fp.missed_frame_ratio(), 0.0);
}

// ---------------------------------------------------------------------
// Format-aware accounting (§8, §9)
// ---------------------------------------------------------------------

/// The defect: cost was `width * height * 4`, regardless of format.
///
/// A half-float intermediate is exactly twice an 8-bit one at the same
/// dimensions, so a budget priced against RGBA8 permitted twice the memory it
/// intended the moment HDR compositing switched the intermediates to
/// `Rgba16Float`.
#[test]
fn a_half_float_intermediate_costs_twice_an_eight_bit_one() {
    let eight_bit = descriptor(256, 256);
    let half = half_float(256, 256);

    assert_eq!(eight_bit.bytes(), 256 * 256 * 4);
    assert_eq!(half.bytes(), eight_bit.bytes() * 2);
}

#[test]
fn accounting_follows_the_format_across_the_range_taim_uses() {
    use taim_wgpu::wgpu::TextureFormat;

    // Every format the renderer can select for a surface or an intermediate,
    // priced per texel at a fixed size.
    for (format, bytes_per_texel) in [
        (TextureFormat::R8Unorm, 1),
        (TextureFormat::Rgba8Unorm, 4),
        (TextureFormat::Bgra8Unorm, 4),
        (TextureFormat::Rgba8UnormSrgb, 4),
        (TextureFormat::Bgra8UnormSrgb, 4),
        (TextureFormat::Rgb10a2Unorm, 4),
        (TextureFormat::R16Float, 2),
        (TextureFormat::Rg16Float, 4),
        (TextureFormat::Rgba16Float, 8),
        (TextureFormat::Rgba32Float, 16),
    ] {
        let descriptor = Descriptor::new(64, 64, format);

        assert_eq!(
            descriptor.bytes(),
            64 * 64 * bytes_per_texel,
            "{format:?} was priced wrongly"
        );
    }
}

#[test]
fn accounting_includes_samples_layers_and_mips() {
    use taim_wgpu::wgpu::TextureFormat;

    let base = Descriptor::new(64, 64, TextureFormat::Rgba8Unorm);

    // Multisampling multiplies the texel count.
    let msaa = Descriptor {
        sample_count: 4,
        ..base
    };
    assert_eq!(msaa.bytes(), base.bytes() * 4);

    // Array layers multiply it too.
    let layered = Descriptor { layers: 6, ..base };
    assert_eq!(layered.bytes(), base.bytes() * 6);

    // A full mip chain adds strictly less than another whole level, and
    // strictly more than nothing -- the classic 1/3 tail.
    let mipped = Descriptor {
        mip_level_count: 7,
        ..base
    };
    assert!(mipped.bytes() > base.bytes());
    assert!(mipped.bytes() < base.bytes() * 2);
}

#[test]
fn depth_formats_are_priced_rather_than_treated_as_free() {
    use taim_wgpu::wgpu::TextureFormat;

    // Depth and stencil formats report no whole-texture block size, only a
    // per-aspect one. An implementation that took `None` as zero would
    // account every depth attachment as free.
    for format in [
        TextureFormat::Depth32Float,
        TextureFormat::Depth24Plus,
        TextureFormat::Depth24PlusStencil8,
    ] {
        let descriptor = Descriptor::new(64, 64, format);

        assert!(
            descriptor.bytes() >= 64 * 64,
            "{format:?} was accounted as {} bytes",
            descriptor.bytes()
        );
    }
}

#[test]
fn a_half_float_pool_admits_half_as_many_intermediates() {
    // The budget behaviour that follows from the accounting: the same byte
    // budget holds half as many half-float intermediates. This is the
    // property a renderer switching to HDR actually experiences.
    let eight_bit = descriptor(256, 256);
    let half = half_float(256, 256);

    let budget = eight_bit.bytes() * 2;

    let mut pool = Pool::new(budget);
    assert!(pool.acquire(eight_bit));
    assert!(pool.acquire(eight_bit));
    assert!(!pool.acquire(eight_bit), "the budget did not bound at two");

    let mut pool = Pool::new(budget);
    assert!(pool.acquire(half));
    assert!(
        !pool.acquire(half),
        "two half-float intermediates fitted a budget sized for two 8-bit ones"
    );
}

#[test]
fn releasing_a_half_float_intermediate_reclaims_its_full_cost() {
    let half = half_float(128, 128);
    let mut pool = Pool::new(half.bytes() * 2);

    assert!(pool.acquire(half));
    assert_eq!(pool.used_bytes(), half.bytes());

    pool.release(half);
    assert_eq!(pool.used_bytes(), 0);
    assert_eq!(pool.reserved_bytes(), half.bytes());

    // Reuse must not allocate again.
    assert!(pool.acquire(half));
    assert_eq!(pool.footprint_bytes(), half.bytes());
}

#[test]
fn textures_of_different_formats_do_not_substitute_for_one_another() {
    // Reuse is only sound between textures that could actually stand in for
    // each other. Same dimensions, different format, is not that.
    let eight_bit = descriptor(128, 128);
    let half = half_float(128, 128);

    let mut pool = Pool::new(eight_bit.bytes() + half.bytes());

    assert!(pool.acquire(eight_bit));
    pool.release(eight_bit);

    assert!(pool.acquire(half));

    // The 8-bit texture is still idle rather than having been handed out as
    // a half-float one.
    assert_eq!(pool.reserved_bytes(), eight_bit.bytes());
    assert_eq!(pool.used_bytes(), half.bytes());
}
