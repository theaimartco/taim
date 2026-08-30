//! Effect damage-expansion, clipping, and sampling-region correctness
//! (`phase.md` Phase 12 exit gate: "effects respect damage expansion and
//! budgets"; effect policy: "never blur the full surface when a smaller
//! sampling region is correct").
//!
//! These are CPU-verifiable geometry/cost properties of the effect
//! model, independent of any GPU device or approved pixel image.

use taim_core::{Point, Rectangle, Size, Vector};
use taim_graphics::quality::Tier;
use taim_wgpu::effects::{Effect, Graph, backdrop_blur::BackdropBlur, blur::Blur, shadow::Shadow};

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rectangle {
    Rectangle::new(Point::new(x, y), Size::new(w, h))
}

#[test]
fn blur_expands_bounds_by_its_radius_on_every_side() {
    let source = rect(100.0, 100.0, 40.0, 40.0);
    // Large surface so no clipping interferes with the expansion check.
    let surface = rect(0.0, 0.0, 1000.0, 1000.0);

    let bounds = Effect::Blur(Blur { radius: 10.0 })
        .bounds(source, surface, Tier::Balanced)
        .expect("bounds intersect the surface");

    // Expanded by radius (10, within Balanced's max of 20) on each side.
    assert_eq!(bounds.x, 90.0);
    assert_eq!(bounds.y, 90.0);
    assert_eq!(bounds.width, 60.0);
    assert_eq!(bounds.height, 60.0);
}

#[test]
fn blur_radius_is_clamped_to_the_tier_limit_when_expanding_bounds() {
    let source = rect(100.0, 100.0, 40.0, 40.0);
    let surface = rect(0.0, 0.0, 1000.0, 1000.0);

    // Request radius 100, but Balanced caps blur radius at 20, so bounds
    // must expand by at most 20 -- an oversized radius cannot expand
    // damage without limit.
    let bounds = Effect::Blur(Blur { radius: 100.0 })
        .bounds(source, surface, Tier::Balanced)
        .expect("bounds intersect the surface");

    assert_eq!(bounds.x, 80.0, "expansion clamped to the tier's max radius");
    assert_eq!(bounds.width, 80.0);
}

#[test]
fn effect_bounds_are_clipped_to_the_surface() {
    // A source at the surface corner with a blur that would spill past it
    // must clip: no effect samples or damages outside the surface.
    let source = rect(0.0, 0.0, 20.0, 20.0);
    let surface = rect(0.0, 0.0, 30.0, 30.0);

    let bounds = Effect::Blur(Blur { radius: 8.0 })
        .bounds(source, surface, Tier::Balanced)
        .expect("intersects the surface");

    assert!(bounds.x >= surface.x, "clipped left edge");
    assert!(bounds.y >= surface.y, "clipped top edge");
    assert!(
        bounds.x + bounds.width <= surface.x + surface.width + f32::EPSILON,
        "clipped right edge"
    );
    assert!(
        bounds.y + bounds.height <= surface.y + surface.height + f32::EPSILON,
        "clipped bottom edge"
    );
}

#[test]
fn an_effect_entirely_off_surface_has_no_bounds() {
    let source = rect(10_000.0, 10_000.0, 10.0, 10.0);
    let surface = rect(0.0, 0.0, 100.0, 100.0);

    assert_eq!(
        Effect::Blur(Blur { radius: 4.0 }).bounds(source, surface, Tier::Balanced),
        None,
        "an effect with no surface overlap contributes no damage at all"
    );
}

#[test]
fn a_small_element_does_not_blur_the_whole_surface() {
    // The core "never blur the full surface when a smaller region is
    // correct" rule: a small element on a large surface must produce a
    // sampling region strictly smaller than the surface.
    let source = rect(100.0, 100.0, 40.0, 40.0);
    let surface = rect(0.0, 0.0, 1920.0, 1080.0);
    let blur = Blur { radius: 12.0 };

    assert!(
        blur.is_bounded_below_surface(source, surface, Tier::Balanced.policy().max_blur_radius),
        "a small element's blur must not cover the whole surface"
    );

    let area = Effect::Blur(blur).sampling_area(source, surface, Tier::Balanced);
    let surface_area = surface.width * surface.height;
    assert!(
        area < surface_area,
        "sampling area must be below full surface"
    );
}

#[test]
fn a_full_surface_element_legitimately_uses_the_whole_surface() {
    // The counter-case: when the element genuinely covers the surface, a
    // full-surface region *is* correct, and the rule must not forbid it.
    let surface = rect(0.0, 0.0, 200.0, 200.0);
    let source = surface;
    let blur = Blur { radius: 4.0 };

    assert!(
        !blur.is_bounded_below_surface(source, surface, Tier::Balanced.policy().max_blur_radius),
        "a full-surface element correctly samples the full surface"
    );
}

#[test]
fn shadow_bounds_include_offset_and_spread() {
    let source = rect(0.0, 0.0, 10.0, 10.0);
    let shadow = Effect::Shadow(Shadow {
        offset: Vector::new(2.0, 3.0),
        blur_radius: 2.0,
        spread: 1.0,
    });

    let bounds = shadow
        .bounds(source, Rectangle::INFINITE, Tier::Balanced)
        .expect("infinite surface always intersects");

    assert!(bounds.x < source.x && bounds.y < source.y);
    assert!(bounds.width > source.width && bounds.height > source.height);
}

#[test]
fn separable_downsampled_blur_is_far_cheaper_than_a_full_kernel() {
    // A wide blur must use downsampling + separable passes, so its cost
    // is dramatically below a naive full 2D kernel over the source.
    let source = rect(0.0, 0.0, 256.0, 256.0);
    let blur = Blur { radius: 20.0 };
    let max = Tier::Balanced.policy().max_blur_radius;

    let separable = blur.separable_sample_cost(source, max);

    let radius = blur.effective_radius(max);
    let full_kernel =
        (source.width * source.height * (2.0 * radius + 1.0) * (2.0 * radius + 1.0)) as u64;

    assert!(
        separable * 4 < full_kernel,
        "separable+downsampled cost {separable} must be far below full-kernel {full_kernel}"
    );
}

#[test]
fn a_larger_radius_uses_a_coarser_downsample_factor() {
    assert_eq!(Blur { radius: 4.0 }.downsample(), 1);
    assert_eq!(Blur { radius: 12.0 }.downsample(), 2);
    assert_eq!(Blur { radius: 24.0 }.downsample(), 4);
}

#[test]
fn recovery_tier_rejects_expensive_blur_nodes() {
    let mut graph = Graph::new();
    assert!(!graph.push(Effect::Blur(Blur { radius: 1.0 }), Tier::Recovery));
    assert!(graph.is_empty());
}

#[test]
fn backdrop_blur_is_rejected_when_the_tier_disables_it() {
    // Efficient disables backdrop blur; the effect must not be admitted,
    // and its clipped sampling area must be zero.
    let source = rect(10.0, 10.0, 50.0, 50.0);
    let surface = rect(0.0, 0.0, 200.0, 200.0);
    let effect = Effect::BackdropBlur(BackdropBlur {
        radius: 8.0,
        area: 2_500.0,
    });

    assert!(!effect.allowed(Tier::Efficient));
    assert_eq!(
        effect.sampling_area(source, surface, Tier::Efficient),
        0.0,
        "a disallowed effect contributes no sampling area"
    );
}
