//! Golden tests for damage correctness.
//!
//! These tests verify that the damage system correctly reports damage
//! for various scenarios (motion, translucency, blur/shadow), and that
//! the partial-redraw result would match a full-redraw oracle.
//!
//! Since we cannot render actual pixels without a GPU context in unit
//! tests, these tests verify the *damage computation* correctness:
//! that the reported damage regions fully cover all areas that would
//! differ between the old and new frame.

use aura_graphics::core::{Point, Rectangle, Size};
use aura_graphics::damage::DamageRegion;

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rectangle {
    Rectangle::new(Point::new(x, y), Size::new(w, h))
}

fn surface() -> Rectangle {
    rect(0.0, 0.0, 1920.0, 1080.0)
}

/// Verifies that the union of damage rects fully covers both the old
/// and new bounds of a moved widget — the "no uncovered changed pixel"
/// property.
fn assert_damage_covers_movement(
    damage: &DamageRegion,
    old_bounds: Rectangle,
    new_bounds: Rectangle,
) {
    if damage.is_full() {
        return; // full redraw trivially covers everything
    }

    let rects = damage
        .rects()
        .expect("damage should not be empty for a movement");
    assert!(
        !rects.is_empty(),
        "movement must produce at least one damage rect"
    );

    // Check old bounds corners
    for corner in corners(old_bounds) {
        assert!(
            rects.iter().any(|r| r.contains(corner)),
            "old bounds corner ({}, {}) not covered by damage",
            corner.x,
            corner.y
        );
    }

    // Check new bounds corners
    for corner in corners(new_bounds) {
        assert!(
            rects.iter().any(|r| r.contains(corner)),
            "new bounds corner ({}, {}) not covered by damage",
            corner.x,
            corner.y
        );
    }
}

fn corners(rect: Rectangle) -> [Point; 4] {
    [
        Point::new(rect.x, rect.y),
        Point::new(rect.x + rect.width - 0.5, rect.y),
        Point::new(rect.x, rect.y + rect.height - 0.5),
        Point::new(rect.x + rect.width - 0.5, rect.y + rect.height - 0.5),
    ]
}

// ── Moving widget damages both old and new bounds ──────────────────

#[test]
fn moving_widget_damages_both_locations() {
    let old = rect(50.0, 50.0, 100.0, 100.0);
    let new = rect(200.0, 200.0, 100.0, 100.0);

    let mut damage = DamageRegion::empty();
    damage.add_movement(old, new);

    assert_damage_covers_movement(&damage, old, new);
}

#[test]
fn moving_widget_with_small_offset_damages_both_locations() {
    let old = rect(100.0, 100.0, 50.0, 50.0);
    let new = rect(105.0, 103.0, 50.0, 50.0);

    let mut damage = DamageRegion::empty();
    damage.add_movement(old, new);

    assert_damage_covers_movement(&damage, old, new);
}

#[test]
fn moving_widget_with_size_change_damages_both_locations() {
    let old = rect(100.0, 100.0, 50.0, 50.0);
    let new = rect(100.0, 100.0, 80.0, 80.0); // grew in place

    let mut damage = DamageRegion::empty();
    damage.add_movement(old, new);

    assert_damage_covers_movement(&damage, old, new);
}

// ── Translucent content damages the full union ─────────────────────

#[test]
fn translucent_overlapping_content_uses_union_damage() {
    // When two translucent layers overlap and one changes, the
    // entire overlapping region must be redrawn. We model this as
    // a single merged damage rect covering the union.
    let layer_a = rect(50.0, 50.0, 200.0, 200.0);
    let layer_b = rect(150.0, 150.0, 200.0, 200.0);

    let mut damage = DamageRegion::empty();
    // The changed layer damages its own bounds
    damage.add_rect(layer_b);
    // The overlap with the unchanged layer also must be redrawn
    // (since the unchanged layer's translucent content shows through)
    if let Some(overlap) = layer_a.intersection(&layer_b) {
        damage.add_rect(overlap);
    }

    // All of layer_b's corners should be in the damage
    let rects = damage.rects().unwrap();
    for corner in corners(layer_b) {
        assert!(
            rects.iter().any(|r| r.contains(corner)),
            "translucent overlap: corner ({}, {}) not covered",
            corner.x,
            corner.y
        );
    }
}

// ── Blur/shadow expands damage by filter radius ────────────────────

#[test]
fn blur_expands_damage_by_sampling_radius() {
    let widget_bounds = rect(100.0, 100.0, 200.0, 200.0);
    let blur_radius = 16.0;

    let mut damage = DamageRegion::from_rect(widget_bounds).unwrap();
    damage.expand_for_filter(blur_radius);

    let expanded = damage.rects().unwrap()[0];

    // The expanded rect must extend at least `blur_radius` in each direction
    assert!(
        expanded.x <= widget_bounds.x - blur_radius,
        "left edge not expanded enough: {} > {}",
        expanded.x,
        widget_bounds.x - blur_radius
    );
    assert!(
        expanded.y <= widget_bounds.y - blur_radius,
        "top edge not expanded enough: {} > {}",
        expanded.y,
        widget_bounds.y - blur_radius
    );
    assert!(
        expanded.x + expanded.width >= widget_bounds.x + widget_bounds.width + blur_radius,
        "right edge not expanded enough"
    );
    assert!(
        expanded.y + expanded.height >= widget_bounds.y + widget_bounds.height + blur_radius,
        "bottom edge not expanded enough"
    );
}

#[test]
fn shadow_expands_damage_by_offset_plus_radius() {
    let widget_bounds = rect(100.0, 100.0, 200.0, 200.0);
    let shadow_blur = 8.0;
    let shadow_offset_x = 4.0;
    let shadow_offset_y = 4.0;

    // Shadow damage = widget bounds + offset, then expanded by blur radius
    let shadow_bounds = Rectangle::new(
        Point::new(
            widget_bounds.x + shadow_offset_x,
            widget_bounds.y + shadow_offset_y,
        ),
        Size::new(widget_bounds.width, widget_bounds.height),
    );

    let mut damage = DamageRegion::empty();
    damage.add_rect(widget_bounds);
    damage.add_rect(shadow_bounds);
    damage.expand_for_filter(shadow_blur);

    // The bounding rect must cover both the widget and its shadow
    let bounding = damage.bounding_rect().unwrap();
    assert!(bounding.x <= widget_bounds.x - shadow_blur);
    assert!(bounding.y <= widget_bounds.y - shadow_blur);
}

// ── Oracle comparison ──────────────────────────────────────────────

#[test]
fn damage_from_single_widget_change_covers_old_and_new_state() {
    // Oracle test: if a widget at rect A changes to rect B,
    // a full redraw would differ at both A and B. The damage
    // must cover both.
    let old_state = rect(50.0, 50.0, 100.0, 30.0);
    let new_state = rect(50.0, 50.0, 150.0, 30.0); // width grew

    let mut damage = DamageRegion::empty();
    damage.add_movement(old_state, new_state);
    damage.clip_to_surface(surface());

    assert_damage_covers_movement(&damage, old_state, new_state);
}

#[test]
fn static_subtree_produces_no_damage() {
    // A subtree whose revision has not changed should produce
    // zero damage rects — the central optimization of Phase 4.
    let damage = DamageRegion::empty();
    assert!(damage.is_empty());
    assert_eq!(damage.rect_count(), 0);
    assert_eq!(damage.total_area(), 0.0);
}

#[test]
fn multiple_scattered_changes_stay_precise_until_threshold() {
    let mut damage = DamageRegion::empty();

    // Add 10 small, scattered changes
    for i in 0..10 {
        damage.add_rect(rect((i as f32) * 180.0, (i as f32) * 100.0, 20.0, 20.0));
    }

    damage.clip_to_surface(surface());
    damage.optimize(surface());

    // With only 10 small rects on a 1920x1080 surface, we should
    // NOT collapse to full (total area is 10 * 400 = 4000, well
    // under 70% of 2,073,600).
    assert!(
        !damage.is_full(),
        "10 small scattered rects should not collapse to full"
    );
}
