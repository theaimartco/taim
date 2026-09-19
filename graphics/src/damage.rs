//! Production damage-region tracking for the Taim rendering pipeline.
//!
//! This module provides the damage tracking system that determines which
//! regions of the surface need redrawing after a frame change. It builds
//! on top of the renderer-neutral vocabulary from [`taim_core::damage`]
//! (Phase 2) to provide concrete region tracking, clipping, transform
//! application, merge heuristics, and full-screen fallback.
//!
//! # Architecture
//!
//! [`DamageRegion`] is the central type: a `SmallVec`-backed collection of
//! rectangles representing the areas that need redrawing. For typical
//! interactive frames (a hovered button, a blinking cursor), damage fits
//! inline in the `SmallVec` with zero heap allocation. Pathological frames
//! (hundreds of scattered updates) are detected and collapsed to a single
//! full-surface redraw rather than growing unbounded.
//!
//! # Invariants (from phase.md)
//!
//! - Damage is clipped to the surface, finite, and normalized.
//! - Moving content damages both its previous and current device-space bounds.
//! - Filters expand damage by their sampling radius.
//! - Too many/scattered rectangles collapse to a full-surface redraw.
//!
//! [`taim_core::damage`]: crate::core::damage

use crate::core::{Point, Rectangle, Size, Transformation};
use smallvec::SmallVec;

/// The inline capacity for damage rectangles.
///
/// Typical interactive frames produce 1–4 damage rects (a cursor blink,
/// a button hover, a progress bar update). Eight gives comfortable
/// headroom before spilling to the heap, while keeping `DamageRegion`
/// itself small enough for stack storage and cheap cloning.
const INLINE_CAPACITY: usize = 8;

/// The maximum number of individual rectangles tracked before collapsing
/// to a full-surface redraw.
///
/// Beyond this count, tracking and compositing individual rects costs
/// more than simply redrawing the entire surface. This bound prevents
/// pathological frames from growing the rect list without limit
/// (phase.md rule: "bound every cache and queue").
const MAX_RECTS: usize = 32;

/// The fraction of total surface area above which damage is collapsed
/// to a full-surface redraw. When the union of all damage rects covers
/// more than this percentage of the surface, partial redraw saves
/// nothing and the overhead of scissoring is wasted.
const FULL_SURFACE_THRESHOLD: f32 = 0.70;

/// The minimum area of a damage rect to be worth tracking individually.
/// Rects smaller than this are noise and can be merged aggressively.
const MIN_RECT_AREA: f32 = 4.0;

/// A set of damaged regions on a surface.
///
/// Tracks which rectangular areas need redrawing after a frame change.
/// Uses `SmallVec` for inline storage of the common case (few rects)
/// and collapses to a full-surface marker when the damage is too
/// scattered or large for partial redraw to be beneficial.
#[derive(Debug, Clone)]
pub struct DamageRegion {
    /// The individual damage rectangles, or empty if this represents
    /// a full-surface redraw (signalled by `is_full`).
    rects: SmallVec<[Rectangle; INLINE_CAPACITY]>,

    /// Whether this region represents a full-surface redraw.
    /// When true, `rects` is empty and all content must be redrawn.
    is_full: bool,
}

impl DamageRegion {
    /// Creates an empty damage region (nothing needs redrawing).
    pub fn empty() -> Self {
        Self {
            rects: SmallVec::new(),
            is_full: false,
        }
    }

    /// Creates a full-surface damage region (everything must be redrawn).
    pub fn full() -> Self {
        Self {
            rects: SmallVec::new(),
            is_full: true,
        }
    }

    /// Creates a damage region from a single rectangle.
    ///
    /// Returns [`None`] if the rectangle is degenerate (zero or negative
    /// area). Returns a full-surface region if any component is
    /// non-finite.
    pub fn from_rect(rect: Rectangle) -> Option<Self> {
        if !rect.x.is_finite()
            || !rect.y.is_finite()
            || !rect.width.is_finite()
            || !rect.height.is_finite()
        {
            return Some(Self::full());
        }

        if rect.width <= 0.0 || rect.height <= 0.0 {
            return None;
        }

        let mut region = Self::empty();
        region.rects.push(rect);
        Some(region)
    }

    /// Returns `true` if nothing needs redrawing.
    pub fn is_empty(&self) -> bool {
        !self.is_full && self.rects.is_empty()
    }

    /// Returns `true` if the entire surface must be redrawn.
    pub fn is_full(&self) -> bool {
        self.is_full
    }

    /// Returns the individual damage rectangles, or `None` if this is
    /// a full-surface redraw.
    pub fn rects(&self) -> Option<&[Rectangle]> {
        if self.is_full {
            None
        } else {
            Some(&self.rects)
        }
    }

    /// Returns the number of individual damage rectangles, or `0` if
    /// this is a full-surface redraw.
    pub fn rect_count(&self) -> usize {
        if self.is_full { 0 } else { self.rects.len() }
    }

    /// Adds a rectangle to the damage region.
    ///
    /// If the region is already full, this is a no-op. If adding this
    /// rect would exceed [`MAX_RECTS`], the region collapses to full.
    pub fn add_rect(&mut self, rect: Rectangle) {
        if self.is_full {
            return;
        }

        // Skip degenerate rects
        if rect.width <= 0.0 || rect.height <= 0.0 {
            return;
        }

        // Non-finite → full surface
        if !rect.x.is_finite()
            || !rect.y.is_finite()
            || !rect.width.is_finite()
            || !rect.height.is_finite()
        {
            self.collapse_to_full();
            return;
        }

        // Skip rects too small to matter
        if rect.area() < MIN_RECT_AREA {
            return;
        }

        // Check rect count bound
        if self.rects.len() >= MAX_RECTS {
            self.collapse_to_full();
            return;
        }

        self.rects.push(rect);
    }

    /// Records damage for content that moved from `old_bounds` to
    /// `new_bounds`.
    ///
    /// Both the previous and current device-space bounds are damaged,
    /// since the old location must be cleared and the new location must
    /// be drawn. This is the correct behavior for any widget that
    /// changes position or size between frames.
    pub fn add_movement(&mut self, old_bounds: Rectangle, new_bounds: Rectangle) {
        self.add_rect(old_bounds);
        self.add_rect(new_bounds);
    }

    /// Records predictive swept damage for content moving with a linear velocity vector over `dt_seconds`.
    ///
    /// Extends the damage area from `current_bounds` along the displacement vector
    /// `(velocity[0] * dt, velocity[1] * dt)`, covering both current and next-frame bounds
    /// into a single swept damage envelope.
    pub fn add_swept_movement(
        &mut self,
        current_bounds: Rectangle,
        velocity: [f32; 2],
        dt_seconds: f32,
    ) {
        if !dt_seconds.is_finite() || dt_seconds <= 0.0 {
            self.add_rect(current_bounds);
            return;
        }

        let dx = velocity[0] * dt_seconds;
        let dy = velocity[1] * dt_seconds;
        if !dx.is_finite() || !dy.is_finite() {
            self.add_rect(current_bounds);
            return;
        }

        let next_bounds = Rectangle {
            x: current_bounds.x + dx,
            y: current_bounds.y + dy,
            width: current_bounds.width,
            height: current_bounds.height,
        };

        let swept = current_bounds.union(&next_bounds);
        self.add_rect(swept);
    }

    /// Records predictive damage using a [`taim_raster::ObjectVelocity`] kinematics descriptor.
    #[cfg(feature = "raster")]
    pub fn add_velocity_movement(
        &mut self,
        current_bounds: Rectangle,
        velocity: taim_raster::ObjectVelocity,
        dt_seconds: f32,
    ) {
        self.add_swept_movement(current_bounds, velocity.linear, dt_seconds);
    }

    /// Expands all existing damage rects outward by the given `radius`,
    /// accounting for filter effects (blur, shadow) that sample pixels
    /// beyond their nominal bounds.
    ///
    /// A blur with radius `r` reads pixels up to `r` units outside its
    /// source rect, so the damage must be expanded by that amount in
    /// every direction to ensure those source pixels are also redrawn.
    pub fn expand_for_filter(&mut self, radius: f32) {
        if self.is_full || radius <= 0.0 {
            return;
        }

        for rect in &mut self.rects {
            *rect = expand_rect(*rect, radius);
        }
    }

    /// Clips all damage rects to the given surface bounds.
    ///
    /// Rects that fall entirely outside the surface are removed.
    /// This must be called before presenting to ensure damage is
    /// within the renderable area.
    pub fn clip_to_surface(&mut self, surface: Rectangle) {
        if self.is_full {
            return;
        }

        self.rects.retain_mut(|rect| {
            if let Some(clipped) = rect.intersection(&surface)
                && clipped.width > 0.0
                && clipped.height > 0.0
            {
                *rect = clipped;
                return true;
            }
            false
        });
    }

    /// Applies a transformation to all damage rects.
    ///
    /// This transforms damage from local coordinates to device
    /// coordinates (or between coordinate spaces). Uses the
    /// `Rectangle * Transformation` operator already defined in
    /// taim_core.
    pub fn apply_transform(&mut self, transform: Transformation) {
        if self.is_full || transform == Transformation::IDENTITY {
            return;
        }

        for rect in &mut self.rects {
            *rect = *rect * transform;
        }
    }

    /// Merges another damage region into this one.
    pub fn merge(&mut self, other: &DamageRegion) {
        if self.is_full {
            return;
        }

        if other.is_full {
            self.collapse_to_full();
            return;
        }

        for rect in &other.rects {
            self.add_rect(*rect);
        }
    }

    /// Merges adjacent or overlapping rects to reduce the total count,
    /// using a greedy union heuristic: two rects are merged if the
    /// wasted area (the union minus the sum of the two originals)
    /// is below a threshold, meaning the overdraw cost is acceptable.
    ///
    /// This should be called after all damage has been accumulated
    /// and before presenting, to reduce the number of scissor rects
    /// the GPU must process.
    pub fn optimize(&mut self, surface: Rectangle) {
        if self.is_full || self.rects.len() <= 1 {
            return;
        }

        // First, check if total damage exceeds the full-surface threshold
        let surface_area = surface.area();
        if surface_area > 0.0 {
            let total_damage: f32 = self.rects.iter().map(Rectangle::area).sum();
            if total_damage / surface_area >= FULL_SURFACE_THRESHOLD {
                self.collapse_to_full();
                return;
            }
        }

        // Sort by distance from origin for spatial locality
        self.rects.sort_by(|a, b| {
            a.center()
                .distance_squared(Point::ORIGIN)
                .total_cmp(&b.center().distance_squared(Point::ORIGIN))
        });

        // Greedy merge: walk through sorted rects and merge nearby ones
        const MERGE_WASTE_THRESHOLD: f32 = 20_000.0;
        let mut merged: SmallVec<[Rectangle; INLINE_CAPACITY]> = SmallVec::new();

        for rect in self.rects.drain(..) {
            let mut was_merged = false;

            for existing in &mut merged {
                let union = existing.union(&rect);
                let waste = union.area() - existing.area() - rect.area();

                if waste <= MERGE_WASTE_THRESHOLD {
                    *existing = union;
                    was_merged = true;
                    break;
                }
            }

            if !was_merged {
                merged.push(rect);
            }
        }

        self.rects = merged;

        // After merging, re-check count
        if self.rects.len() > MAX_RECTS {
            self.collapse_to_full();
        }
    }

    /// Returns the bounding rectangle of all damage, or `None` if empty.
    pub fn bounding_rect(&self) -> Option<Rectangle> {
        if self.is_full || self.rects.is_empty() {
            return None;
        }

        let mut result = self.rects[0];
        for rect in &self.rects[1..] {
            result = result.union(rect);
        }
        Some(result)
    }

    /// Returns the total damaged area (sum of individual rect areas).
    pub fn total_area(&self) -> f32 {
        if self.is_full {
            return f32::INFINITY;
        }

        self.rects.iter().map(Rectangle::area).sum()
    }

    /// Converts a [`core::damage::Region`] into a [`DamageRegion`].
    pub fn from_core_region(region: crate::core::damage::Region) -> Self {
        match region {
            crate::core::damage::Region::Full => Self::full(),
            crate::core::damage::Region::Area(rect) => {
                Self::from_rect(rect).unwrap_or_else(Self::empty)
            }
        }
    }

    fn collapse_to_full(&mut self) {
        self.is_full = true;
        self.rects.clear();
    }
}

impl Default for DamageRegion {
    fn default() -> Self {
        Self::empty()
    }
}

/// Expands a rectangle outward by `radius` in every direction.
fn expand_rect(rect: Rectangle, radius: f32) -> Rectangle {
    Rectangle::new(
        Point::new(rect.x - radius, rect.y - radius),
        Size::new(rect.width + radius * 2.0, rect.height + radius * 2.0),
    )
}

// ── Legacy API ─────────────────────────────────────────────────────────
//
// The functions below preserve the existing public API from the original
// `graphics::damage` module so that all existing callers (the wgpu and
// tiny_skia renderers) continue to compile without changes.

/// Diffs the damage regions given some previous and current primitives.
pub fn diff<T>(
    previous: &[T],
    current: &[T],
    bounds: impl Fn(&T) -> Vec<Rectangle>,
    diff: impl Fn(&T, &T) -> Vec<Rectangle>,
) -> Vec<Rectangle> {
    let damage = previous.iter().zip(current).flat_map(|(a, b)| diff(a, b));

    if previous.len() == current.len() {
        damage.collect()
    } else {
        let (smaller, bigger) = if previous.len() < current.len() {
            (previous, current)
        } else {
            (current, previous)
        };

        // Extend damage by the added/removed primitives
        damage
            .chain(bigger[smaller.len()..].iter().flat_map(bounds))
            .collect()
    }
}

/// Computes the damage regions given some previous and current primitives.
pub fn list<T>(
    previous: &[T],
    current: &[T],
    bounds: impl Fn(&T) -> Vec<Rectangle>,
    are_equal: impl Fn(&T, &T) -> bool,
) -> Vec<Rectangle> {
    diff(previous, current, &bounds, |a, b| {
        if are_equal(a, b) {
            vec![]
        } else {
            bounds(a).into_iter().chain(bounds(b)).collect()
        }
    })
}

/// Groups the given damage regions that are close together inside the given
/// bounds.
pub fn group(mut damage: Vec<Rectangle>, bounds: Rectangle) -> Vec<Rectangle> {
    const AREA_THRESHOLD: f32 = 20_000.0;

    damage.sort_by(|a, b| {
        a.center()
            .distance_squared(Point::ORIGIN)
            .total_cmp(&b.center().distance_squared(Point::ORIGIN))
    });

    let mut output = Vec::new();
    let mut scaled = damage
        .into_iter()
        .filter_map(|region| region.intersection(&bounds))
        .filter(|region| region.width >= 1.0 && region.height >= 1.0);

    if let Some(mut current) = scaled.next() {
        for region in scaled {
            let union = current.union(&region);

            if union.area() - current.area() - region.area() <= AREA_THRESHOLD {
                current = union;
            } else {
                output.push(current);
                current = region;
            }
        }

        output.push(current);
    }

    output
}

#[cfg(test)]
mod damage_region_tests {
    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rectangle {
        Rectangle::new(Point::new(x, y), Size::new(w, h))
    }

    fn surface() -> Rectangle {
        rect(0.0, 0.0, 1920.0, 1080.0)
    }

    #[test]
    fn empty_region_has_no_rects_and_is_not_full() {
        let region = DamageRegion::empty();
        assert!(region.is_empty());
        assert!(!region.is_full());
        assert_eq!(region.rect_count(), 0);
    }

    #[test]
    fn full_region_is_full_and_has_no_individual_rects() {
        let region = DamageRegion::full();
        assert!(!region.is_empty());
        assert!(region.is_full());
        assert!(region.rects().is_none());
    }

    #[test]
    fn from_rect_creates_a_single_rect_region() {
        let r = rect(10.0, 20.0, 100.0, 50.0);
        let region = DamageRegion::from_rect(r).unwrap();

        assert!(!region.is_empty());
        assert!(!region.is_full());
        assert_eq!(region.rect_count(), 1);
        assert_eq!(region.rects().unwrap()[0], r);
    }

    #[test]
    fn from_rect_rejects_degenerate_rects() {
        assert!(DamageRegion::from_rect(rect(0.0, 0.0, 0.0, 10.0)).is_none());
        assert!(DamageRegion::from_rect(rect(0.0, 0.0, -5.0, 10.0)).is_none());
    }

    #[test]
    fn from_rect_falls_back_to_full_on_non_finite() {
        let r = rect(f32::NAN, 0.0, 10.0, 10.0);
        let region = DamageRegion::from_rect(r).unwrap();
        assert!(region.is_full());
    }

    #[test]
    fn add_rect_accumulates_rects() {
        let mut region = DamageRegion::empty();
        region.add_rect(rect(0.0, 0.0, 10.0, 10.0));
        region.add_rect(rect(50.0, 50.0, 20.0, 20.0));
        assert_eq!(region.rect_count(), 2);
    }

    #[test]
    fn add_rect_skips_degenerate() {
        let mut region = DamageRegion::empty();
        region.add_rect(rect(0.0, 0.0, 0.0, 10.0));
        assert!(region.is_empty());
    }

    #[test]
    fn add_rect_collapses_to_full_on_non_finite() {
        let mut region = DamageRegion::empty();
        region.add_rect(rect(0.0, 0.0, 10.0, 10.0));
        region.add_rect(rect(f32::INFINITY, 0.0, 10.0, 10.0));
        assert!(region.is_full());
    }

    #[test]
    fn exceeding_max_rects_collapses_to_full() {
        let mut region = DamageRegion::empty();
        for i in 0..=MAX_RECTS {
            region.add_rect(rect(i as f32 * 100.0, 0.0, 10.0, 10.0));
        }
        assert!(region.is_full());
    }

    #[test]
    fn add_movement_damages_old_and_new_bounds() {
        let mut region = DamageRegion::empty();
        let old = rect(0.0, 0.0, 50.0, 50.0);
        let new = rect(100.0, 100.0, 50.0, 50.0);
        region.add_movement(old, new);

        assert_eq!(region.rect_count(), 2);
        let rects = region.rects().unwrap();
        assert_eq!(rects[0], old);
        assert_eq!(rects[1], new);
    }

    #[test]
    fn expand_for_filter_grows_rects_by_radius() {
        let mut region = DamageRegion::from_rect(rect(10.0, 10.0, 20.0, 20.0)).unwrap();
        region.expand_for_filter(5.0);

        let rects = region.rects().unwrap();
        assert_eq!(rects[0].x, 5.0);
        assert_eq!(rects[0].y, 5.0);
        assert_eq!(rects[0].width, 30.0);
        assert_eq!(rects[0].height, 30.0);
    }

    #[test]
    fn expand_for_filter_is_noop_for_zero_radius() {
        let original = rect(10.0, 10.0, 20.0, 20.0);
        let mut region = DamageRegion::from_rect(original).unwrap();
        region.expand_for_filter(0.0);

        assert_eq!(region.rects().unwrap()[0], original);
    }

    #[test]
    fn clip_to_surface_removes_out_of_bounds_rects() {
        let mut region = DamageRegion::empty();
        region.add_rect(rect(-100.0, -100.0, 50.0, 50.0)); // fully outside
        region.add_rect(rect(10.0, 10.0, 20.0, 20.0)); // fully inside

        region.clip_to_surface(surface());

        assert_eq!(region.rect_count(), 1);
        assert_eq!(region.rects().unwrap()[0], rect(10.0, 10.0, 20.0, 20.0));
    }

    #[test]
    fn clip_to_surface_clips_partially_outside_rects() {
        let mut region = DamageRegion::from_rect(rect(-5.0, -5.0, 20.0, 20.0)).unwrap();

        let surface_bounds = rect(0.0, 0.0, 100.0, 100.0);
        region.clip_to_surface(surface_bounds);

        let rects = region.rects().unwrap();
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].x, 0.0);
        assert_eq!(rects[0].y, 0.0);
        assert_eq!(rects[0].width, 15.0);
        assert_eq!(rects[0].height, 15.0);
    }

    #[test]
    fn clip_to_surface_is_noop_for_full() {
        let mut region = DamageRegion::full();
        region.clip_to_surface(surface());
        assert!(region.is_full());
    }

    #[test]
    fn apply_transform_is_noop_for_identity() {
        let original = rect(10.0, 10.0, 20.0, 20.0);
        let mut region = DamageRegion::from_rect(original).unwrap();
        region.apply_transform(Transformation::IDENTITY);
        assert_eq!(region.rects().unwrap()[0], original);
    }

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
    fn optimize_collapses_to_full_when_damage_exceeds_threshold() {
        let mut region = DamageRegion::empty();
        // Add rects covering >70% of the surface (1920*1080 = 2,073,600)
        // Need multiple rects to get past the len <= 1 early return
        region.add_rect(rect(0.0, 0.0, 1920.0, 400.0)); // 768,000
        region.add_rect(rect(0.0, 400.0, 1920.0, 400.0)); // 768,000
        // Total: 1,536,000 / 2,073,600 = 74% > 70%
        region.optimize(surface());
        assert!(region.is_full());
    }

    #[test]
    fn optimize_merges_adjacent_rects() {
        let mut region = DamageRegion::empty();
        // Two adjacent rects should merge
        region.add_rect(rect(0.0, 0.0, 10.0, 10.0));
        region.add_rect(rect(10.0, 0.0, 10.0, 10.0));

        region.optimize(surface());

        assert_eq!(region.rect_count(), 1);
    }

    #[test]
    fn bounding_rect_returns_union_of_all_rects() {
        let mut region = DamageRegion::empty();
        region.add_rect(rect(0.0, 0.0, 10.0, 10.0));
        region.add_rect(rect(20.0, 20.0, 10.0, 10.0));

        let bounds = region.bounding_rect().unwrap();
        assert_eq!(bounds, rect(0.0, 0.0, 30.0, 30.0));
    }

    #[test]
    fn total_area_sums_individual_rects() {
        let mut region = DamageRegion::empty();
        region.add_rect(rect(0.0, 0.0, 10.0, 10.0));
        region.add_rect(rect(50.0, 50.0, 20.0, 20.0));
        assert_eq!(region.total_area(), 100.0 + 400.0);
    }

    #[test]
    fn total_area_of_full_is_infinite() {
        assert!(DamageRegion::full().total_area().is_infinite());
    }

    #[test]
    fn from_core_region_full_makes_full() {
        let region = DamageRegion::from_core_region(crate::core::damage::Region::Full);
        assert!(region.is_full());
    }

    #[test]
    fn from_core_region_area_makes_rect_region() {
        let core_region = crate::core::damage::Region::area(rect(10.0, 10.0, 50.0, 50.0)).unwrap();
        let region = DamageRegion::from_core_region(core_region);
        assert_eq!(region.rect_count(), 1);
    }

    #[test]
    fn adding_to_a_full_region_is_noop() {
        let mut region = DamageRegion::full();
        region.add_rect(rect(0.0, 0.0, 10.0, 10.0));
        assert!(region.is_full());
        assert_eq!(region.rect_count(), 0);
    }

    #[test]
    fn min_rect_area_filters_tiny_rects() {
        let mut region = DamageRegion::empty();
        // 1x1 = 1.0 area, below MIN_RECT_AREA of 4.0
        region.add_rect(rect(0.0, 0.0, 1.0, 1.0));
        assert!(region.is_empty());
    }

    #[test]
    fn predictive_swept_damage_expands_bounds_along_trajectory() {
        let mut region = DamageRegion::empty();
        let initial = rect(10.0, 10.0, 20.0, 20.0);
        region.add_swept_movement(initial, [100.0, 50.0], 0.1);

        assert_eq!(region.rect_count(), 1);
        let swept = region.bounding_rect().unwrap();
        assert_eq!(swept.x, 10.0);
        assert_eq!(swept.y, 10.0);
        assert_eq!(swept.width, 30.0);
        assert_eq!(swept.height, 25.0);
    }

    #[test]
    fn predictive_swept_damage_handles_negative_velocity() {
        let mut region = DamageRegion::empty();
        let initial = rect(50.0, 50.0, 20.0, 20.0);
        region.add_swept_movement(initial, [-200.0, -100.0], 0.1);

        assert_eq!(region.rect_count(), 1);
        let swept = region.bounding_rect().unwrap();
        assert_eq!(swept.x, 30.0);
        assert_eq!(swept.y, 40.0);
        assert_eq!(swept.width, 40.0);
        assert_eq!(swept.height, 30.0);
    }

    #[test]
    fn predictive_swept_damage_safe_under_zero_or_invalid_dt() {
        let mut region = DamageRegion::empty();
        let initial = rect(10.0, 10.0, 20.0, 20.0);
        region.add_swept_movement(initial, [100.0, 50.0], 0.0);
        assert_eq!(region.bounding_rect().unwrap(), initial);

        let mut region2 = DamageRegion::empty();
        region2.add_swept_movement(initial, [f32::NAN, 50.0], 0.1);
        assert_eq!(region2.bounding_rect().unwrap(), initial);
    }

    #[cfg(feature = "raster")]
    #[test]
    fn predictive_velocity_movement_delegates_to_swept_bounds() {
        let mut region = DamageRegion::empty();
        let initial = rect(0.0, 0.0, 50.0, 50.0);
        let velocity = taim_raster::ObjectVelocity {
            linear: [100.0, 200.0],
            angular: 0.0,
            scale_rate: 0.0,
        };
        region.add_velocity_movement(initial, velocity, 0.5);

        assert_eq!(region.rect_count(), 1);
        let swept = region.bounding_rect().unwrap();
        assert_eq!(swept.x, 0.0);
        assert_eq!(swept.y, 0.0);
        assert_eq!(swept.width, 100.0);
        assert_eq!(swept.height, 150.0);
    }
}

#[cfg(test)]
mod legacy_api_tests {
    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rectangle {
        Rectangle::new(Point::new(x, y), Size::new(w, h))
    }
    #[test]
    fn diff_with_equal_length_produces_changed_bounds() {
        let previous = vec![rect(0.0, 0.0, 10.0, 10.0)];
        let current = vec![rect(5.0, 5.0, 10.0, 10.0)];

        let damage = list(&previous, &current, |r| vec![*r], |a, b| a == b);
        assert_eq!(damage.len(), 2); // old + new bounds
    }

    #[test]
    fn group_merges_close_rects() {
        let damage = vec![rect(0.0, 0.0, 10.0, 10.0), rect(10.0, 0.0, 10.0, 10.0)];

        let grouped = group(damage, rect(0.0, 0.0, 1000.0, 1000.0));
        assert_eq!(grouped.len(), 1);
    }
}
