//! Integration tests for the damage region tracking system.
//!
//! These tests verify the damage region behavior from a consumer's
//! perspective, exercising the public API of [`aura_graphics::damage`]
//! including merge, clip, transform, threshold, and filter expansion
//! operations.

use aura_graphics::core::{Point, Rectangle, Size, Transformation};
use aura_graphics::damage::DamageRegion;

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rectangle {
    Rectangle::new(Point::new(x, y), Size::new(w, h))
}

fn surface() -> Rectangle {
    rect(0.0, 0.0, 1920.0, 1080.0)
}

// ── Construction ───────────────────────────────────────────────────

#[test]
fn empty_region_reports_no_damage() {
    let region = DamageRegion::empty();
    assert!(region.is_empty());
    assert!(!region.is_full());
    assert_eq!(region.rect_count(), 0);
    assert_eq!(region.total_area(), 0.0);
}

#[test]
fn full_region_reports_full_surface() {
    let region = DamageRegion::full();
    assert!(!region.is_empty());
    assert!(region.is_full());
    assert!(region.total_area().is_infinite());
    assert!(region.rects().is_none());
}

#[test]
fn from_rect_with_valid_rect_creates_single_rect_region() {
    let r = rect(10.0, 20.0, 100.0, 50.0);
    let region = DamageRegion::from_rect(r).unwrap();

    assert!(!region.is_empty());
    assert!(!region.is_full());
    assert_eq!(region.rect_count(), 1);

    let rects = region.rects().unwrap();
    assert_eq!(rects[0], r);
}

#[test]
fn from_rect_rejects_zero_width_or_height() {
    assert!(DamageRegion::from_rect(rect(0.0, 0.0, 0.0, 50.0)).is_none());
    assert!(DamageRegion::from_rect(rect(0.0, 0.0, 50.0, 0.0)).is_none());
}

#[test]
fn from_rect_rejects_negative_dimensions() {
    assert!(DamageRegion::from_rect(rect(0.0, 0.0, -10.0, 50.0)).is_none());
}

#[test]
fn from_rect_falls_back_to_full_on_nan() {
    let r = rect(f32::NAN, 0.0, 10.0, 10.0);
    let region = DamageRegion::from_rect(r).unwrap();
    assert!(region.is_full());
}

#[test]
fn from_rect_falls_back_to_full_on_infinity() {
    let r = rect(0.0, 0.0, f32::INFINITY, 10.0);
    let region = DamageRegion::from_rect(r).unwrap();
    assert!(region.is_full());
}

// ── Accumulation ───────────────────────────────────────────────────

#[test]
fn add_rect_accumulates_multiple_rects() {
    let mut region = DamageRegion::empty();
    region.add_rect(rect(0.0, 0.0, 50.0, 50.0));
    region.add_rect(rect(100.0, 100.0, 50.0, 50.0));
    region.add_rect(rect(200.0, 200.0, 50.0, 50.0));
    assert_eq!(region.rect_count(), 3);
}

#[test]
fn add_rect_silently_ignores_degenerate_rects() {
    let mut region = DamageRegion::empty();
    region.add_rect(rect(0.0, 0.0, 0.0, 10.0));
    region.add_rect(rect(0.0, 0.0, -5.0, 10.0));
    assert!(region.is_empty());
}

#[test]
fn add_rect_is_noop_when_already_full() {
    let mut region = DamageRegion::full();
    region.add_rect(rect(0.0, 0.0, 10.0, 10.0));
    assert!(region.is_full());
    assert_eq!(region.rect_count(), 0);
}

#[test]
fn add_rect_collapses_to_full_on_non_finite_input() {
    let mut region = DamageRegion::empty();
    region.add_rect(rect(0.0, 0.0, 50.0, 50.0));
    region.add_rect(rect(f32::NAN, 0.0, 10.0, 10.0));
    assert!(region.is_full());
}

#[test]
fn exceeding_max_rects_collapses_to_full() {
    let mut region = DamageRegion::empty();
    // Add more than MAX_RECTS (32) individual rects
    for i in 0..=32 {
        region.add_rect(rect(i as f32 * 100.0, 0.0, 10.0, 10.0));
    }
    assert!(region.is_full());
}

// ── Movement ───────────────────────────────────────────────────────

#[test]
fn add_movement_damages_both_old_and_new_bounds() {
    let mut region = DamageRegion::empty();
    let old = rect(0.0, 0.0, 50.0, 50.0);
    let new = rect(100.0, 100.0, 50.0, 50.0);
    region.add_movement(old, new);

    assert_eq!(region.rect_count(), 2);

    let rects = region.rects().unwrap();
    assert!(rects.contains(&old));
    assert!(rects.contains(&new));
}

#[test]
fn add_movement_with_identical_bounds_still_adds_two_rects() {
    let mut region = DamageRegion::empty();
    let bounds = rect(10.0, 10.0, 50.0, 50.0);
    region.add_movement(bounds, bounds);
    // Both old and new are added (they'll merge later during optimize)
    assert_eq!(region.rect_count(), 2);
}

// ── Filter expansion ───────────────────────────────────────────────

#[test]
fn expand_for_filter_grows_rects_uniformly() {
    let mut region = DamageRegion::from_rect(rect(20.0, 20.0, 40.0, 40.0)).unwrap();
    region.expand_for_filter(10.0);

    let rects = region.rects().unwrap();
    assert_eq!(rects[0].x, 10.0);
    assert_eq!(rects[0].y, 10.0);
    assert_eq!(rects[0].width, 60.0);
    assert_eq!(rects[0].height, 60.0);
}

#[test]
fn expand_for_filter_is_noop_for_zero_radius() {
    let original = rect(10.0, 10.0, 20.0, 20.0);
    let mut region = DamageRegion::from_rect(original).unwrap();
    region.expand_for_filter(0.0);

    assert_eq!(region.rects().unwrap()[0], original);
}

#[test]
fn expand_for_filter_is_noop_for_negative_radius() {
    let original = rect(10.0, 10.0, 20.0, 20.0);
    let mut region = DamageRegion::from_rect(original).unwrap();
    region.expand_for_filter(-5.0);

    assert_eq!(region.rects().unwrap()[0], original);
}

#[test]
fn expand_for_filter_is_noop_for_full() {
    let mut region = DamageRegion::full();
    region.expand_for_filter(10.0);
    assert!(region.is_full());
}

// ── Clipping ───────────────────────────────────────────────────────

#[test]
fn clip_removes_fully_outside_rects() {
    let mut region = DamageRegion::empty();
    region.add_rect(rect(-200.0, -200.0, 50.0, 50.0)); // fully outside
    region.add_rect(rect(10.0, 10.0, 20.0, 20.0)); // fully inside
    region.clip_to_surface(surface());

    assert_eq!(region.rect_count(), 1);
    assert_eq!(region.rects().unwrap()[0], rect(10.0, 10.0, 20.0, 20.0));
}

#[test]
fn clip_trims_partially_outside_rects() {
    let mut region = DamageRegion::from_rect(rect(-5.0, -5.0, 20.0, 20.0)).unwrap();

    let surface_bounds = rect(0.0, 0.0, 100.0, 100.0);
    region.clip_to_surface(surface_bounds);

    let rects = region.rects().unwrap();
    assert_eq!(rects.len(), 1);
    // Clipped to [0, 0] - [15, 15]
    assert_eq!(rects[0].x, 0.0);
    assert_eq!(rects[0].y, 0.0);
    assert_eq!(rects[0].width, 15.0);
    assert_eq!(rects[0].height, 15.0);
}

#[test]
fn clip_is_noop_for_full_region() {
    let mut region = DamageRegion::full();
    region.clip_to_surface(surface());
    assert!(region.is_full());
}

#[test]
fn clip_produces_empty_when_all_rects_outside() {
    let mut region = DamageRegion::empty();
    region.add_rect(rect(-100.0, -100.0, 10.0, 10.0));
    region.clip_to_surface(rect(0.0, 0.0, 50.0, 50.0));
    assert!(region.is_empty());
}

// ── Transform ──────────────────────────────────────────────────────

#[test]
fn apply_identity_transform_is_noop() {
    let original = rect(10.0, 10.0, 20.0, 20.0);
    let mut region = DamageRegion::from_rect(original).unwrap();
    region.apply_transform(Transformation::IDENTITY);
    assert_eq!(region.rects().unwrap()[0], original);
}

#[test]
fn apply_transform_is_noop_for_full() {
    let mut region = DamageRegion::full();
    region.apply_transform(Transformation::translate(10.0, 10.0));
    assert!(region.is_full());
}

// ── Merge ──────────────────────────────────────────────────────────

#[test]
fn merge_combines_two_regions() {
    let mut a = DamageRegion::from_rect(rect(0.0, 0.0, 10.0, 10.0)).unwrap();
    let b = DamageRegion::from_rect(rect(50.0, 50.0, 20.0, 20.0)).unwrap();
    a.merge(&b);
    assert_eq!(a.rect_count(), 2);
}

#[test]
fn merge_with_full_makes_self_full() {
    let mut a = DamageRegion::from_rect(rect(0.0, 0.0, 10.0, 10.0)).unwrap();
    let b = DamageRegion::full();
    a.merge(&b);
    assert!(a.is_full());
}

#[test]
fn merge_with_empty_is_noop() {
    let mut a = DamageRegion::from_rect(rect(0.0, 0.0, 10.0, 10.0)).unwrap();
    let b = DamageRegion::empty();
    a.merge(&b);
    assert_eq!(a.rect_count(), 1);
}

#[test]
fn merge_full_with_anything_stays_full() {
    let mut a = DamageRegion::full();
    let b = DamageRegion::from_rect(rect(0.0, 0.0, 10.0, 10.0)).unwrap();
    a.merge(&b);
    assert!(a.is_full());
}

// ── Optimize ───────────────────────────────────────────────────────

#[test]
fn optimize_merges_adjacent_rects() {
    let mut region = DamageRegion::empty();
    region.add_rect(rect(0.0, 0.0, 10.0, 10.0));
    region.add_rect(rect(10.0, 0.0, 10.0, 10.0));
    region.optimize(surface());
    // Adjacent rects with small waste should merge
    assert_eq!(region.rect_count(), 1);
}

#[test]
fn optimize_collapses_to_full_when_damage_exceeds_threshold() {
    let mut region = DamageRegion::empty();
    // Add multiple rects covering >70% of the surface (1920*1080 = 2,073,600)
    region.add_rect(rect(0.0, 0.0, 1920.0, 400.0));
    region.add_rect(rect(0.0, 400.0, 1920.0, 400.0));
    region.optimize(surface());
    assert!(region.is_full());
}

#[test]
fn optimize_preserves_scattered_rects_under_threshold() {
    let mut region = DamageRegion::empty();
    // Two small, distant rects — their union would waste too much area
    region.add_rect(rect(0.0, 0.0, 10.0, 10.0));
    region.add_rect(rect(1900.0, 1060.0, 10.0, 10.0));
    region.optimize(surface());
    // These are far apart; union wastes > threshold → stay separate
    assert_eq!(region.rect_count(), 2);
}

#[test]
fn optimize_is_noop_for_single_rect() {
    let mut region = DamageRegion::from_rect(rect(10.0, 10.0, 50.0, 50.0)).unwrap();
    region.optimize(surface());
    assert_eq!(region.rect_count(), 1);
}

#[test]
fn optimize_is_noop_for_full() {
    let mut region = DamageRegion::full();
    region.optimize(surface());
    assert!(region.is_full());
}

// ── Bounding rect ──────────────────────────────────────────────────

#[test]
fn bounding_rect_of_multiple_rects_is_their_union() {
    let mut region = DamageRegion::empty();
    region.add_rect(rect(0.0, 0.0, 10.0, 10.0));
    region.add_rect(rect(20.0, 20.0, 10.0, 10.0));

    let bounds = region.bounding_rect().unwrap();
    assert_eq!(bounds.x, 0.0);
    assert_eq!(bounds.y, 0.0);
    assert_eq!(bounds.width, 30.0);
    assert_eq!(bounds.height, 30.0);
}

#[test]
fn bounding_rect_of_empty_is_none() {
    assert!(DamageRegion::empty().bounding_rect().is_none());
}

#[test]
fn bounding_rect_of_full_is_none() {
    assert!(DamageRegion::full().bounding_rect().is_none());
}

// ── Total area ─────────────────────────────────────────────────────

#[test]
fn total_area_sums_all_rects() {
    let mut region = DamageRegion::empty();
    region.add_rect(rect(0.0, 0.0, 10.0, 10.0)); // 100
    region.add_rect(rect(50.0, 50.0, 20.0, 20.0)); // 400
    assert_eq!(region.total_area(), 500.0);
}

#[test]
fn total_area_of_full_is_infinite() {
    assert!(DamageRegion::full().total_area().is_infinite());
}

#[test]
fn total_area_of_empty_is_zero() {
    assert_eq!(DamageRegion::empty().total_area(), 0.0);
}

// ── Core interop ───────────────────────────────────────────────────

#[test]
fn from_core_region_full_produces_full() {
    let region = DamageRegion::from_core_region(aura_graphics::core::damage::Region::Full);
    assert!(region.is_full());
}

#[test]
fn from_core_region_area_produces_rect_region() {
    let core_region =
        aura_graphics::core::damage::Region::area(rect(10.0, 10.0, 50.0, 50.0)).unwrap();
    let region = DamageRegion::from_core_region(core_region);
    assert_eq!(region.rect_count(), 1);
}

// ── Legacy API ─────────────────────────────────────────────────────

#[test]
fn legacy_group_merges_close_rects() {
    let damage = vec![rect(0.0, 0.0, 10.0, 10.0), rect(10.0, 0.0, 10.0, 10.0)];

    let grouped = aura_graphics::damage::group(damage, rect(0.0, 0.0, 1000.0, 1000.0));
    assert_eq!(grouped.len(), 1);
}

#[test]
fn legacy_group_keeps_distant_rects_separate() {
    let damage = vec![rect(0.0, 0.0, 10.0, 10.0), rect(900.0, 900.0, 10.0, 10.0)];

    let grouped = aura_graphics::damage::group(damage, rect(0.0, 0.0, 1000.0, 1000.0));
    assert_eq!(grouped.len(), 2);
}

#[test]
fn legacy_diff_detects_changed_primitives() {
    let previous = vec![rect(0.0, 0.0, 10.0, 10.0)];
    let current = vec![rect(5.0, 5.0, 10.0, 10.0)];

    let damage = aura_graphics::damage::list(&previous, &current, |r| vec![*r], |a, b| a == b);
    assert_eq!(damage.len(), 2); // old + new bounds
}

// ── Property: no uncovered changed pixel ───────────────────────────

#[test]
fn damage_always_covers_moved_content() {
    // For any movement from old to new bounds, the union of the
    // damage rects must contain both the old and new bounds.
    let old = rect(10.0, 10.0, 50.0, 50.0);
    let new = rect(100.0, 200.0, 50.0, 50.0);

    let mut region = DamageRegion::empty();
    region.add_movement(old, new);

    let bounding = region.bounding_rect().unwrap();
    assert!(bounding.contains(Point::new(old.x, old.y)));
    assert!(bounding.contains(Point::new(
        old.x + old.width - 1.0,
        old.y + old.height - 1.0
    )));
    assert!(bounding.contains(Point::new(new.x, new.y)));
    assert!(bounding.contains(Point::new(
        new.x + new.width - 1.0,
        new.y + new.height - 1.0
    )));
}

#[test]
fn expanded_damage_covers_filter_sampling_radius() {
    let radius = 8.0;
    let source = rect(50.0, 50.0, 100.0, 100.0);

    let mut region = DamageRegion::from_rect(source).unwrap();
    region.expand_for_filter(radius);

    let expanded = region.rects().unwrap()[0];
    assert!(expanded.x <= source.x - radius);
    assert!(expanded.y <= source.y - radius);
    assert!(expanded.width >= source.width + 2.0 * radius);
    assert!(expanded.height >= source.height + 2.0 * radius);
}
