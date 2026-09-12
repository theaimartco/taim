//! Dirty-flag propagation, bounded damage aggregation, and phase
//! counter wiring for the incremental widget tree.
//!
//! Three related but separable pieces of "what changed, and how much
//! work does that actually cost" live here:
//!
//! - [`propagate`] walks a [`Tree`] and pushes [`DirtyFlags::PAINT`] up
//!   to every ancestor of a node that changed, since a container's own
//!   cached paint state (`Tree::cached_transform`/`cached_clip`) can
//!   depend on a descendant having repainted, even when the container's
//!   own layout and semantics are untouched. It never pushes
//!   [`DirtyFlags::LAYOUT`] or [`DirtyFlags::SEMANTICS`] upward: a
//!   child's layout or semantics changing does not, by itself, prove an
//!   ancestor's own layout or semantics are stale (that is each
//!   container's own `Widget::layout`/`Widget::semantics`
//!   implementation's responsibility to decide, same as today).
//! - [`Aggregator`] merges [`damage::Request`]s the way
//!   [`taim_core::Shell::request_damage`] already collects them, except
//!   bounded: once more than [`Aggregator::MAX_REGIONS`] distinct areas
//!   have accumulated, it gives up trying to track them individually and
//!   reports [`damage::Region::Full`] instead. Unbounded per-region
//!   tracking is a real, documented risk (`phase.md` rule: "bound every
//!   cache and queue") -- a pathological frame that touches thousands of
//!   scattered small widgets must not grow an `Aggregator` proportionally
//!   to that count.
//! - Both call into [`diagnostics::Counters`] at the point they can prove
//!   a phase actually ran (or was skipped via a cache hit), which is the
//!   "phase scheduling" observability [`Counters`] existed for since
//!   Phase 2 but that nothing yet called.
//!
//! [`Counters`]: crate::diagnostics::Counters

use crate::core::damage;
use crate::core::widget::tree::DirtyFlags;
use crate::core::widget::{Id, Tree};
use crate::diagnostics::{self, Counters};

/// Marks `root`, every node along `path` (a sequence of child indices
/// from `root`, outermost first, as produced by
/// [`crate::hit_test::Path`]), and every ancestor in between,
/// [`DirtyFlags::PAINT`]-dirty.
///
/// An empty `path` marks only `root` itself. `propagate` is the
/// *upward* half of invalidation: a node deep in the tree changing its
/// own paint output does not, by itself, dirty its ancestors' cached
/// paint state (`Tree::cached_transform`/`cached_clip`), and nothing
/// else in this codebase currently does that walk back up.
pub fn propagate(root: &mut Tree, path: &[usize]) {
    let mut node = root;
    node.mark_dirty(DirtyFlags::PAINT);

    for &index in path {
        node = &mut node.children[index];
        node.mark_dirty(DirtyFlags::PAINT);
    }
}

/// Merges [`damage::Request`]s into a single, bounded [`damage::Region`],
/// falling back to [`damage::Region::Full`] once more than
/// [`Aggregator::MAX_REGIONS`] distinct areas have been recorded.
///
/// This mirrors [`taim_core::Shell::damage_requests`]' accumulation
/// (append-only, one entry per widget-reported request) but adds the
/// bound that vocabulary deliberately left to a later phase: `Shell`
/// itself does no merging or capping, by design (Phase 2's "define the
/// vocabulary, not the tracking strategy"). `Aggregator` is that
/// tracking strategy, for callers (see `docs/architecture/widget-lifecycle.md`)
/// that need a single answer -- "redraw this rectangle" or "redraw
/// everything" -- rather than a raw list.
#[derive(Debug)]
pub struct Aggregator {
    merged: Option<damage::Region>,
    area_count: usize,
    /// Every cause that contributed.
    ///
    /// Kept because `Region` cannot carry it: a first version of this type
    /// read `request.region` and dropped `request.cause` on the floor, so
    /// a full redraw arrived at the caller with no reason attached — the
    /// one case where the reason matters most, and the one case where
    /// something is definitely wrong.
    causes: damage::Causes,
    /// Why precision was abandoned, if it was.
    fallback: Option<damage::Fallback>,
}

impl Aggregator {
    /// The number of distinct areas [`Aggregator`] will track before
    /// giving up and reporting [`damage::Region::Full`].
    ///
    /// Chosen so a handful of scattered widget updates (a typical
    /// interactive frame: a hovered button, a blinking cursor, a
    /// progress bar) stay precise, while a frame that touches hundreds
    /// of small, disjoint regions (e.g. every row in a large list)
    /// degrades to one full-surface redraw rather than growing this
    /// structure -- and the per-frame rectangle-union work it would take
    /// to merge them -- without bound.
    pub const MAX_REGIONS: usize = 64;

    /// Creates a new, empty [`Aggregator`].
    pub const fn new() -> Self {
        Self {
            merged: None,
            area_count: 0,
            causes: damage::Causes::empty(),
            fallback: None,
        }
    }

    /// Records a [`damage::Request`], merging its [`damage::Region`]
    /// into the running total.
    pub fn record(&mut self, request: damage::Request) {
        self.record_with(request, None);
    }

    /// Records a [`damage::Request`], additionally incrementing
    /// [`diagnostics::Phase::DamageFallback`] on `counters` if this call
    /// is the one that causes the fallback to [`damage::Region::Full`]
    /// by exceeding [`Aggregator::MAX_REGIONS`] -- distinct from every
    /// other reason a frame ends up fully redrawing (an explicit
    /// `Region::Full` request, a layout invalidation, ...), which this
    /// method does not count, since those are not this [`Aggregator`]'s
    /// own decision.
    pub fn record_with_counters(&mut self, request: damage::Request, counters: &Counters) {
        self.record_with(request, Some(counters));
    }

    fn record_with(&mut self, request: damage::Request, counters: Option<&Counters>) {
        // Recorded for every request, including ones that change nothing
        // about the region: a frame that was already `Full` still had
        // reasons, and dropping them once the region saturated would make
        // the busiest frames the least explicable.
        self.causes |= damage::Causes::of(request.cause);

        match request.region {
            damage::Region::Full => {
                // The *first* full request wins the explanation. A later
                // one did not cause the fallback -- the region was already
                // full -- and overwriting would name whichever widget
                // happened to run last.
                if self.fallback.is_none() {
                    self.fallback = Some(damage::Fallback::Requested(request.cause));
                }

                self.merged = Some(damage::Region::Full);
            }
            area @ damage::Region::Area(_) => {
                if matches!(self.merged, Some(damage::Region::Full)) {
                    return;
                }

                if self.area_count >= Self::MAX_REGIONS {
                    self.merged = Some(damage::Region::Full);
                    self.fallback = Some(damage::Fallback::Overflow {
                        areas: self.area_count,
                    });

                    if let Some(counters) = counters {
                        counters.increment(diagnostics::Phase::DamageFallback);
                    }

                    return;
                }

                self.area_count += 1;
                self.merged = Some(match self.merged.take() {
                    Some(existing) => existing.merge(area),
                    None => area,
                });
            }
        }
    }

    /// Records every [`damage::Request`] in `requests`, as a convenience
    /// over calling [`Aggregator::record`] in a loop (e.g. directly on
    /// [`taim_core::Shell::damage_requests`]'s output).
    pub fn record_all<'a>(&mut self, requests: impl IntoIterator<Item = &'a damage::Request>) {
        for request in requests {
            self.record(*request);
        }
    }

    /// Returns the merged [`damage::Region`], or `None` if nothing has
    /// been recorded yet.
    pub fn region(&self) -> Option<damage::Region> {
        self.merged
    }

    /// Returns the merged damage *and why*, or `None` if nothing has been
    /// recorded yet.
    ///
    /// Prefer this to [`Aggregator::region`] anywhere the answer might be
    /// [`damage::Region::Full`]. "Redraw everything" is not a finding;
    /// "redraw everything because 64 scattered areas overflowed" is.
    pub fn attribution(&self) -> Option<damage::Attribution> {
        Some(damage::Attribution {
            region: self.merged?,
            causes: self.causes,
            fallback: self.fallback,
        })
    }

    /// Every cause recorded so far.
    pub fn causes(&self) -> damage::Causes {
        self.causes
    }
}

impl Default for Aggregator {
    fn default() -> Self {
        Self::new()
    }
}

/// Increments [`diagnostics::Phase::PaintPreparation`] on `counters` if
/// `computed` is `true` (the widget actually redrew), leaving `counters`
/// untouched otherwise (a clean frame did no work, so there is nothing
/// to count for this phase) -- the concrete "how much work did we avoid"
/// number `Counters`'s own module documentation asks for.
pub fn record_paint(counters: &Counters, computed: bool) {
    if computed {
        counters.increment(diagnostics::Phase::PaintPreparation);
    }
}

/// The fundamental reason an element or region was scheduled to repaint (Pillar XIV).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepaintReason {
    /// A reactive signal dependency changed value.
    SignalChanged(String),
    /// A parent container invalidated its subtree.
    ParentInvalidation,
    /// Layout constraints changed, altering bounds.
    LayoutConstraintShift,
    /// Damage accumulation overflowed maximum regions to full redraw.
    DamageOverflow,
    /// Continuous animation or transition tick.
    AnimationTick,
    /// Global theme revision or scheme changed.
    ThemeRevisionChanged,
    /// Direct pointer gesture or hover interaction.
    PointerInteraction,
    /// Explicit request or external shell trigger.
    ExplicitRequest(String),
}

/// Attribution entry answering "Why did this repaint?" for a widget or tree node (Pillar XIV).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepaintCausality {
    /// Target widget ID, if known.
    pub widget_id: Option<Id>,
    /// Dirty flags triggered by this change.
    pub dirty_flags: DirtyFlags,
    /// Primary causality reason.
    pub reason: RepaintReason,
    /// Associated human-readable description or signal name.
    pub description: String,
}

/// A tracker recording per-frame repaint causality for live inspector diagnostics (Pillar XIV).
#[derive(Debug, Default, Clone)]
pub struct RepaintTracker {
    entries: Vec<RepaintCausality>,
}

impl RepaintTracker {
    /// Creates a new empty [`RepaintTracker`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a new repaint causality event.
    pub fn record(&mut self, causality: RepaintCausality) {
        self.entries.push(causality);
    }

    /// Answers "Why did this repaint?" for a specific widget.
    pub fn why_did_it_repaint(&self, id: &Id) -> Option<&RepaintCausality> {
        self.entries
            .iter()
            .rev()
            .find(|c| c.widget_id.as_ref() == Some(id))
    }

    /// Returns all recorded causality entries for the current frame.
    pub fn all_causalities(&self) -> &[RepaintCausality] {
        &self.entries
    }

    /// Clears the tracker at frame boundaries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod propagate_tests {
    use super::*;

    #[test]
    fn propagating_an_empty_path_marks_only_the_root() {
        let mut root = Tree::with_children(vec![Tree::empty()]);
        root.clear_dirty(DirtyFlags::all());
        root.children[0].clear_dirty(DirtyFlags::all());

        propagate(&mut root, &[]);

        assert!(root.dirty().contains(DirtyFlags::PAINT));
        assert!(!root.children[0].dirty().contains(DirtyFlags::PAINT));
    }

    #[test]
    fn propagating_a_path_marks_the_target_and_every_ancestor() {
        let mut root = Tree::with_children(vec![Tree::with_children(vec![
            Tree::empty(),
            Tree::empty(),
        ])]);
        root.clear_dirty(DirtyFlags::all());
        root.children[0].clear_dirty(DirtyFlags::all());
        root.children[0].children[0].clear_dirty(DirtyFlags::all());
        root.children[0].children[1].clear_dirty(DirtyFlags::all());

        propagate(&mut root, &[0, 1]);

        assert!(root.dirty().contains(DirtyFlags::PAINT));
        assert!(root.children[0].dirty().contains(DirtyFlags::PAINT));
        assert!(
            root.children[0].children[1]
                .dirty()
                .contains(DirtyFlags::PAINT)
        );
        assert!(
            !root.children[0].children[0]
                .dirty()
                .contains(DirtyFlags::PAINT),
            "an unrelated sibling must not be marked dirty"
        );
    }

    #[test]
    fn propagate_never_marks_layout_or_semantics_dirty() {
        let mut root = Tree::with_children(vec![Tree::empty()]);
        root.clear_dirty(DirtyFlags::all());
        root.children[0].clear_dirty(DirtyFlags::all());

        propagate(&mut root, &[0]);

        assert!(!root.dirty().contains(DirtyFlags::LAYOUT));
        assert!(!root.dirty().contains(DirtyFlags::SEMANTICS));
        assert!(!root.children[0].dirty().contains(DirtyFlags::LAYOUT));
        assert!(!root.children[0].dirty().contains(DirtyFlags::SEMANTICS));
    }
}

#[cfg(test)]
mod aggregator_tests {
    use super::*;
    use crate::core::damage::{Cause, Region, Request};
    use crate::core::{Point, Rectangle, Size};

    fn area(x: f32, y: f32, size: f32) -> Region {
        Region::area(Rectangle::new(Point::new(x, y), Size::new(size, size))).unwrap()
    }

    #[test]
    fn an_empty_aggregator_reports_no_region() {
        let aggregator = Aggregator::new();

        assert_eq!(aggregator.region(), None);
    }

    #[test]
    fn a_single_area_request_is_reported_as_is() {
        let mut aggregator = Aggregator::new();
        let region = area(0.0, 0.0, 10.0);

        aggregator.record(Request::new(region, Cause::Content));

        assert_eq!(aggregator.region(), Some(region));
    }

    #[test]
    fn two_area_requests_merge_to_their_union() {
        let mut aggregator = Aggregator::new();

        aggregator.record(Request::new(area(0.0, 0.0, 10.0), Cause::Content));
        aggregator.record(Request::new(area(20.0, 20.0, 10.0), Cause::Content));

        assert_eq!(
            aggregator.region(),
            Some(Region::Area(Rectangle::new(
                Point::new(0.0, 0.0),
                Size::new(30.0, 30.0)
            )))
        );
    }

    #[test]
    fn any_full_request_makes_the_aggregator_report_full() {
        let mut aggregator = Aggregator::new();

        aggregator.record(Request::new(area(0.0, 0.0, 10.0), Cause::Content));
        aggregator.record(Request::full(Cause::Layout));

        assert_eq!(aggregator.region(), Some(Region::Full));
    }

    #[test]
    fn once_full_further_area_requests_do_not_change_the_result() {
        let mut aggregator = Aggregator::new();

        aggregator.record(Request::full(Cause::Layout));
        aggregator.record(Request::new(area(0.0, 0.0, 10.0), Cause::Content));

        assert_eq!(aggregator.region(), Some(Region::Full));
    }

    #[test]
    fn exceeding_max_regions_falls_back_to_full() {
        let mut aggregator = Aggregator::new();

        for i in 0..Aggregator::MAX_REGIONS {
            aggregator.record(Request::new(
                area(i as f32 * 100.0, 0.0, 1.0),
                Cause::Content,
            ));
        }

        assert_ne!(
            aggregator.region(),
            Some(Region::Full),
            "exactly MAX_REGIONS distinct areas must still be tracked precisely"
        );

        aggregator.record(Request::new(
            area(Aggregator::MAX_REGIONS as f32 * 100.0, 0.0, 1.0),
            Cause::Content,
        ));

        assert_eq!(
            aggregator.region(),
            Some(Region::Full),
            "one more than MAX_REGIONS must trigger the full-surface fallback"
        );
    }

    #[test]
    fn record_all_records_every_request_in_order() {
        let mut aggregator = Aggregator::new();
        let requests = [
            Request::new(area(0.0, 0.0, 10.0), Cause::Content),
            Request::new(area(20.0, 20.0, 10.0), Cause::Content),
        ];

        aggregator.record_all(&requests);

        assert_eq!(
            aggregator.region(),
            Some(Region::Area(Rectangle::new(
                Point::new(0.0, 0.0),
                Size::new(30.0, 30.0)
            )))
        );
    }

    #[test]
    fn record_with_counters_increments_damage_fallback_only_on_the_triggering_call() {
        let counters = Counters::new();
        let mut aggregator = Aggregator::new();

        for i in 0..Aggregator::MAX_REGIONS {
            aggregator.record_with_counters(
                Request::new(area(i as f32 * 100.0, 0.0, 1.0), Cause::Content),
                &counters,
            );
        }

        if Counters::is_active() {
            assert_eq!(counters.get(diagnostics::Phase::DamageFallback), 0);
        }

        aggregator.record_with_counters(
            Request::new(
                area(Aggregator::MAX_REGIONS as f32 * 100.0, 0.0, 1.0),
                Cause::Content,
            ),
            &counters,
        );

        if Counters::is_active() {
            assert_eq!(counters.get(diagnostics::Phase::DamageFallback), 1);
        }

        // Every subsequent area request is already `Full`; it must not
        // keep incrementing the counter on every call.
        aggregator
            .record_with_counters(Request::new(area(0.0, 0.0, 1.0), Cause::Content), &counters);

        if Counters::is_active() {
            assert_eq!(counters.get(diagnostics::Phase::DamageFallback), 1);
        }
    }
}

#[cfg(test)]
mod counter_wiring_tests {
    use super::*;

    #[test]
    fn record_paint_increments_only_when_something_was_actually_computed() {
        let counters = Counters::new();

        record_paint(&counters, false);
        record_paint(&counters, true);
        record_paint(&counters, true);

        if Counters::is_active() {
            assert_eq!(counters.get(diagnostics::Phase::PaintPreparation), 2);
        }
    }
}

#[cfg(test)]
mod repaint_tracker_tests {
    use super::*;

    #[test]
    fn answers_why_did_it_repaint_with_exact_signal_path() {
        let mut tracker = RepaintTracker::new();
        let target_id = Id::unique();

        tracker.record(RepaintCausality {
            widget_id: Some(target_id.clone()),
            dirty_flags: DirtyFlags::PAINT,
            reason: RepaintReason::SignalChanged("user_avatar_url".into()),
            description: "Signal 'user_avatar_url' mutated".into(),
        });

        let causality = tracker
            .why_did_it_repaint(&target_id)
            .expect("should find causality");
        assert_eq!(causality.dirty_flags, DirtyFlags::PAINT);
        assert_eq!(
            causality.reason,
            RepaintReason::SignalChanged("user_avatar_url".into())
        );

        // Missing widget query returns None
        assert!(tracker.why_did_it_repaint(&Id::unique()).is_none());

        // Frame clear empties recorded entries
        tracker.clear();
        assert!(tracker.all_causalities().is_empty());
    }
}
