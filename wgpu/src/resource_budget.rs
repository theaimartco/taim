//! Aggregate GPU resource byte accounting and pressure signaling.
//!
//! Individual caches (e.g. [`crate::graphics::text::cache::Cache`])
//! already enforce their own [`core::cache::Budget`] locally. This
//! module answers a different question: across *every* GPU-resident
//! resource this renderer holds (text shaping, image atlases, and any
//! future category), how close is the combined total to becoming a
//! real memory problem? A single cache staying under its own budget
//! does not guarantee the sum of every cache is reasonable -- this is
//! the aggregate view phase.md's Phase 5 step 7 ("texture/buffer/atlas
//! byte accounting and pressure signals") asks for.
//!
//! [`Tracker`] does not evict anything itself (each cache still owns
//! its own eviction policy); it only aggregates what each category
//! reports and classifies the result into a [`Pressure`] level a caller
//! can react to (e.g. by proactively trimming caches harder, or
//! declining to prefetch further resources).

use crate::core::cache::Class;

use std::collections::BTreeMap;

/// How close aggregate GPU resource usage is to a configured limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pressure {
    /// Usage is comfortably within budget.
    Normal,
    /// Usage has crossed the configured warning threshold; a caller
    /// should consider trimming caches more aggressively or declining
    /// to prefetch further resources.
    Elevated,
    /// Usage has crossed the configured critical threshold; a caller
    /// should actively evict, not merely stop growing.
    Critical,
}

/// Tracks aggregate byte usage across GPU resource categories,
/// classified by [`Class`] so a caller under [`Pressure::Critical`] can
/// choose to evict the most disposable category first.
#[derive(Debug)]
pub struct Tracker {
    by_class: BTreeMap<Class, u64>,
    limit_bytes: u64,
    elevated_ratio: f32,
    critical_ratio: f32,
}

impl Tracker {
    /// The default total byte limit across every tracked category: 256 MiB.
    ///
    /// Generous for a typical desktop UI's combined text-shaping and
    /// image-atlas footprint, while still bounding runaway growth from a
    /// pathological scene (e.g. thousands of distinct large images).
    pub const DEFAULT_LIMIT: u64 = 256 * 1024 * 1024;

    /// Creates a new [`Tracker`] with [`Tracker::DEFAULT_LIMIT`] and
    /// default elevated (70%) / critical (90%) thresholds.
    pub fn new() -> Self {
        Self::with_limit(Self::DEFAULT_LIMIT)
    }

    /// Creates a new [`Tracker`] with the given total byte limit and
    /// default elevated (70%) / critical (90%) thresholds.
    pub fn with_limit(limit_bytes: u64) -> Self {
        Self {
            by_class: BTreeMap::new(),
            limit_bytes,
            elevated_ratio: 0.7,
            critical_ratio: 0.9,
        }
    }

    /// Records the current byte usage for a [`Class`], replacing
    /// whatever was previously recorded for it.
    ///
    /// Callers report their own cache's *current total*, not a delta:
    /// each cache already knows its own current byte usage (e.g.
    /// `text::cache::Cache::current_bytes`), so `Tracker` only needs the
    /// latest snapshot per category, not an event stream to replay.
    pub fn report(&mut self, class: Class, bytes: u64) {
        let _ = self.by_class.insert(class, bytes);
    }

    /// Returns the total bytes currently reported across every category.
    pub fn total_bytes(&self) -> u64 {
        self.by_class.values().sum()
    }

    /// Returns the bytes reported for a specific [`Class`], or `0` if
    /// nothing has been reported for it yet.
    pub fn bytes_for(&self, class: Class) -> u64 {
        self.by_class.get(&class).copied().unwrap_or(0)
    }

    /// Returns the configured total byte limit.
    pub fn limit_bytes(&self) -> u64 {
        self.limit_bytes
    }

    /// Returns the current [`Pressure`] level given everything reported
    /// so far.
    pub fn pressure(&self) -> Pressure {
        let ratio = self.total_bytes() as f64 / self.limit_bytes as f64;

        if ratio >= f64::from(self.critical_ratio) {
            Pressure::Critical
        } else if ratio >= f64::from(self.elevated_ratio) {
            Pressure::Elevated
        } else {
            Pressure::Normal
        }
    }

    /// Returns every tracked [`Class`] currently reporting nonzero
    /// bytes, ordered from most to least disposable (matching
    /// [`Class`]'s own ordering) -- the order a caller under
    /// [`Pressure::Critical`] should consider evicting in.
    pub fn classes_by_disposability(&self) -> Vec<Class> {
        let mut classes: Vec<Class> = self
            .by_class
            .iter()
            .filter(|(_, bytes)| **bytes > 0)
            .map(|(class, _)| *class)
            .collect();

        classes.sort();
        classes
    }
}

impl Default for Tracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_tracker_reports_zero_and_normal_pressure() {
        let tracker = Tracker::new();

        assert_eq!(tracker.total_bytes(), 0);
        assert_eq!(tracker.pressure(), Pressure::Normal);
    }

    #[test]
    fn reporting_a_class_updates_its_total_and_the_aggregate() {
        let mut tracker = Tracker::new();

        tracker.report(Class::Essential, 1024);

        assert_eq!(tracker.bytes_for(Class::Essential), 1024);
        assert_eq!(tracker.total_bytes(), 1024);
    }

    #[test]
    fn reporting_a_class_again_replaces_rather_than_accumulates() {
        let mut tracker = Tracker::new();

        tracker.report(Class::Retained, 1000);
        tracker.report(Class::Retained, 500);

        assert_eq!(tracker.bytes_for(Class::Retained), 500);
        assert_eq!(tracker.total_bytes(), 500);
    }

    #[test]
    fn multiple_classes_sum_into_the_total() {
        let mut tracker = Tracker::new();

        tracker.report(Class::Essential, 100);
        tracker.report(Class::Retained, 200);
        tracker.report(Class::Prefetched, 300);

        assert_eq!(tracker.total_bytes(), 600);
    }

    #[test]
    fn pressure_stays_normal_under_the_elevated_threshold() {
        let mut tracker = Tracker::with_limit(1000);

        tracker.report(Class::Essential, 600);

        assert_eq!(tracker.pressure(), Pressure::Normal);
    }

    #[test]
    fn pressure_becomes_elevated_past_seventy_percent() {
        let mut tracker = Tracker::with_limit(1000);

        tracker.report(Class::Essential, 750);

        assert_eq!(tracker.pressure(), Pressure::Elevated);
    }

    #[test]
    fn pressure_becomes_critical_past_ninety_percent() {
        let mut tracker = Tracker::with_limit(1000);

        tracker.report(Class::Essential, 950);

        assert_eq!(tracker.pressure(), Pressure::Critical);
    }

    #[test]
    fn classes_by_disposability_orders_prefetched_before_essential() {
        let mut tracker = Tracker::new();

        tracker.report(Class::Essential, 100);
        tracker.report(Class::Prefetched, 100);
        tracker.report(Class::Retained, 100);

        assert_eq!(
            tracker.classes_by_disposability(),
            vec![Class::Prefetched, Class::Retained, Class::Essential]
        );
    }

    #[test]
    fn classes_with_zero_bytes_are_excluded() {
        let mut tracker = Tracker::new();

        tracker.report(Class::Essential, 100);
        tracker.report(Class::Retained, 0);

        assert_eq!(tracker.classes_by_disposability(), vec![Class::Essential]);
    }

    #[test]
    fn pressure_ordering_is_normal_less_than_elevated_less_than_critical() {
        assert!(Pressure::Normal < Pressure::Elevated);
        assert!(Pressure::Elevated < Pressure::Critical);
    }
}
