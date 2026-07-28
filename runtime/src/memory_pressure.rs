//! Giving memory back, in an order chosen rather than stumbled into.
//!
//! # The order is the design
//!
//! `phase.md`'s recovery sequence starts:
//!
//! 1. Stop optional prefetch/decode.
//! 2. Evict inactive image/effect/display-list caches.
//! 3. Reduce quality tier with hysteresis.
//!
//! That order is not arbitrary, and it is the whole content of this
//! module. Each step is chosen because it is **cheaper to undo than the
//! next one**:
//!
//! - Cancelling a prefetch costs the user nothing. They never asked for
//!   it; it was speculation. It is free to stop and free to restart.
//! - Evicting an *inactive* cache costs a rebuild only if the user comes
//!   back to it, and the whole premise of a cache is that a rebuild is
//!   possible.
//! - Dropping the quality tier is visible. The user sees it. It is the
//!   first step with a cost they can perceive, which is why it is third
//!   and not first.
//!
//! A system that reached for the quality tier first would look worse
//! while still holding megabytes of speculative decode buffers it never
//! needed. That is the failure this module exists to prevent, and it is a
//! *policy* failure — the individual caches were all working correctly.
//!
//! # Why purging is not automatic
//!
//! [`Policy`] decides; it does not act. It returns the [`Purge`] a caller
//! should perform, rather than reaching into caches itself.
//!
//! That split matters because the caller is the only one who knows what
//! is *inactive*. "Evict inactive image caches" is not a decision this
//! module can make — it does not know which image is on screen. It knows
//! only the order to try things in, and that is exactly what it returns.

use crate::core::time::{Duration, Instant};

/// How much trouble the system is in.
///
/// Ordered, so `>=` means "at least this bad" and a caller can compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Level {
    /// Memory is fine.
    #[default]
    Normal,

    /// Memory is tight. Give back what costs the user nothing.
    Moderate,

    /// Memory is short. Give back what costs a rebuild.
    Critical,

    /// The next allocation may fail. Give back everything that is not
    /// needed to draw the current frame.
    ///
    /// The distinction from [`Level::Critical`] is not severity for its
    /// own sake: at `Critical` the system is still trying to stay
    /// pleasant, and here it has stopped. A user would rather see a
    /// blurry frame than have the shell killed.
    Imminent,
}

/// One thing a caller should give back.
///
/// Ordered by what it costs the *user*, cheapest first — which is not the
/// same as what it returns in bytes, and that is the point. Freeing the
/// most memory first would drop the quality tier before cancelling a
/// prefetch nobody asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Purge {
    /// Cancel speculative work not yet needed: prefetch, background
    /// decode, predictive layout.
    ///
    /// Costs the user nothing — they never asked for it.
    Speculative,

    /// Evict caches for content that is not currently visible.
    ///
    /// Costs a rebuild, but only if the user comes back to it.
    Inactive,

    /// Drop the visual quality tier.
    ///
    /// The first step the user can see. Third, not first, for exactly
    /// that reason.
    Quality,

    /// Evict caches for content that *is* visible, forcing it to be
    /// rebuilt this frame.
    ///
    /// Costs a visible stutter. Only at [`Level::Imminent`], where the
    /// alternative is the allocator failing.
    Active,
}

impl Purge {
    /// Every purge, in the order they should be attempted.
    pub const ORDER: [Purge; 4] = [
        Purge::Speculative,
        Purge::Inactive,
        Purge::Quality,
        Purge::Active,
    ];
}

/// How long a [`Policy`] waits before easing off after pressure drops.
///
/// Recovery is slow on purpose. Memory pressure oscillates — freeing
/// memory relieves the pressure that caused the freeing — so a policy
/// that restarted prefetching the instant things looked better would
/// spend its life thrashing between the two states, and each swing costs
/// a visible quality change.
pub const DEFAULT_SETTLE: Duration = Duration::from_secs(10);

/// Decides what to give back, and when to stop.
#[derive(Debug, Clone)]
pub struct Policy {
    level: Level,
    /// The most severe level seen since the last settle. Recovery walks
    /// back from here rather than jumping straight to `Normal`.
    high_water: Level,
    changed_at: Instant,
    settle: Duration,
}

impl Policy {
    /// A policy at [`Level::Normal`].
    pub fn new(now: Instant) -> Self {
        Self {
            level: Level::Normal,
            high_water: Level::Normal,
            changed_at: now,
            settle: DEFAULT_SETTLE,
        }
    }

    /// Sets how long this policy waits before easing off.
    #[must_use]
    pub fn with_settle(mut self, settle: Duration) -> Self {
        self.settle = settle;
        self
    }

    /// The current level.
    pub fn level(&self) -> Level {
        self.level
    }

    /// Reports the system's memory `level`, returning what to purge now.
    ///
    /// Escalation is **immediate**; de-escalation waits for the settle
    /// period. That asymmetry is deliberate and is the same shape as the
    /// quality controller's hysteresis: being slow to free memory risks
    /// the allocator failing, and being slow to *resume* risks nothing but
    /// a few seconds of conservatism.
    pub fn update(&mut self, now: Instant, level: Level) -> Vec<Purge> {
        if level > self.level {
            // Worse. React now -- waiting to free memory is how an
            // allocation fails.
            self.level = level;
            self.high_water = self.high_water.max(level);
            self.changed_at = now;

            return self.purges();
        }

        if level < self.level && now.duration_since(self.changed_at) >= self.settle {
            // Better, and it has stayed better. Ease off one step rather
            // than all the way: freeing memory relieves the pressure that
            // caused the freeing, so "things look fine now" is exactly
            // what a system in trouble looks like immediately after it
            // frees something.
            self.level = self.ease();
            self.changed_at = now;
        }

        self.purges()
    }

    /// One step back from the current level.
    fn ease(&self) -> Level {
        match self.level {
            Level::Imminent => Level::Critical,
            Level::Critical => Level::Moderate,
            Level::Moderate | Level::Normal => Level::Normal,
        }
    }

    /// What to purge at the current level.
    ///
    /// Cumulative: `Critical` does everything `Moderate` does and more. A
    /// policy that swapped one purge for another as it escalated would
    /// have the system *restart* prefetching on its way into an OOM.
    pub fn purges(&self) -> Vec<Purge> {
        match self.level {
            Level::Normal => Vec::new(),
            Level::Moderate => vec![Purge::Speculative],
            Level::Critical => vec![Purge::Speculative, Purge::Inactive, Purge::Quality],
            Level::Imminent => Purge::ORDER.to_vec(),
        }
    }

    /// Whether speculative work (prefetch, background decode) may run.
    ///
    /// The question a caller asks *before* starting work, rather than
    /// after. Starting a prefetch and immediately cancelling it has
    /// already cost the allocation.
    pub fn allows_speculation(&self) -> bool {
        self.level == Level::Normal
    }

    /// Whether the system is under enough pressure to give up visible
    /// quality.
    pub fn demands_quality_drop(&self) -> bool {
        self.level >= Level::Critical
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn epoch() -> Instant {
        use std::sync::OnceLock;
        static EPOCH: OnceLock<Instant> = OnceLock::new();

        *EPOCH.get_or_init(Instant::now)
    }

    fn at(millis: u64) -> Instant {
        epoch() + Duration::from_millis(millis)
    }

    #[test]
    fn nothing_is_purged_when_memory_is_fine() {
        let mut policy = Policy::new(at(0));

        assert!(policy.update(at(0), Level::Normal).is_empty());
        assert!(policy.allows_speculation());
    }

    #[test]
    fn the_cheapest_thing_goes_first() {
        // The whole point of the module: a system that dropped quality
        // before cancelling a prefetch would look worse while still
        // holding megabytes of speculation nobody asked for.
        let mut policy = Policy::new(at(0));

        assert_eq!(
            policy.update(at(0), Level::Moderate),
            vec![Purge::Speculative],
            "moderate pressure must cost the user nothing"
        );
        assert!(!policy.demands_quality_drop());
    }

    #[test]
    fn quality_is_given_up_only_after_the_free_things_are_gone() {
        let mut policy = Policy::new(at(0));

        let purges = policy.update(at(0), Level::Critical);

        assert_eq!(
            purges,
            vec![Purge::Speculative, Purge::Inactive, Purge::Quality]
        );

        let quality = purges.iter().position(|p| *p == Purge::Quality).unwrap();
        let speculative = purges
            .iter()
            .position(|p| *p == Purge::Speculative)
            .unwrap();

        assert!(
            speculative < quality,
            "the visible cost must come after the invisible ones"
        );
    }

    #[test]
    fn visible_content_is_evicted_only_when_the_allocator_is_about_to_fail() {
        // Costs a stutter. The alternative is the shell being killed, and
        // a user would rather see a blurry frame.
        let mut policy = Policy::new(at(0));

        assert!(
            !policy
                .update(at(0), Level::Critical)
                .contains(&Purge::Active)
        );
        assert!(
            policy
                .update(at(0), Level::Imminent)
                .contains(&Purge::Active)
        );
    }

    #[test]
    fn purges_are_cumulative_not_exchanged() {
        // A policy that swapped one purge for another as it escalated
        // would have the system *restart* prefetching on its way into an
        // OOM.
        let mut policy = Policy::new(at(0));

        let moderate = policy.update(at(0), Level::Moderate);
        let critical = policy.update(at(1), Level::Critical);
        let imminent = policy.update(at(2), Level::Imminent);

        for purge in &moderate {
            assert!(
                critical.contains(purge),
                "{purge:?} was dropped at Critical"
            );
        }

        for purge in &critical {
            assert!(
                imminent.contains(purge),
                "{purge:?} was dropped at Imminent"
            );
        }
    }

    #[test]
    fn escalation_is_immediate() {
        // Waiting to free memory is how an allocation fails.
        let mut policy = Policy::new(at(0)).with_settle(Duration::from_secs(10));

        let _ = policy.update(at(0), Level::Moderate);
        let purges = policy.update(at(1), Level::Imminent);

        assert_eq!(policy.level(), Level::Imminent);
        assert!(purges.contains(&Purge::Active));
    }

    #[test]
    fn de_escalation_waits_for_the_pressure_to_stay_gone() {
        // Freeing memory relieves the pressure that caused the freeing, so
        // "things look fine now" is exactly what a system in trouble looks
        // like immediately after it frees something.
        let mut policy = Policy::new(at(0)).with_settle(Duration::from_millis(100));

        let _ = policy.update(at(0), Level::Critical);
        let _ = policy.update(at(10), Level::Normal);

        assert_eq!(
            policy.level(),
            Level::Critical,
            "a policy must not ease off the instant things look better"
        );

        let _ = policy.update(at(200), Level::Normal);

        assert_eq!(policy.level(), Level::Moderate, "and then only one step");
    }

    #[test]
    fn recovery_walks_back_one_step_at_a_time() {
        let mut policy = Policy::new(at(0)).with_settle(Duration::from_millis(100));

        let _ = policy.update(at(0), Level::Imminent);

        for (now, expected) in [
            (200, Level::Critical),
            (400, Level::Moderate),
            (600, Level::Normal),
        ] {
            let _ = policy.update(at(now), Level::Normal);

            assert_eq!(policy.level(), expected, "at {now}ms");
        }
    }

    #[test]
    fn speculation_resumes_only_at_normal() {
        // The question is asked *before* starting work: beginning a
        // prefetch and immediately cancelling it has already cost the
        // allocation.
        let mut policy = Policy::new(at(0)).with_settle(Duration::from_millis(100));

        let _ = policy.update(at(0), Level::Moderate);
        assert!(!policy.allows_speculation());

        let _ = policy.update(at(200), Level::Normal);
        assert!(policy.allows_speculation());
    }

    #[test]
    fn staying_at_a_level_keeps_purging_it() {
        // The caller acts on the returned list every time it asks. A
        // policy that reported a purge once and then went quiet would have
        // newly-created speculative work survive.
        let mut policy = Policy::new(at(0));

        assert_eq!(
            policy.update(at(0), Level::Moderate),
            vec![Purge::Speculative]
        );
        assert_eq!(
            policy.update(at(1), Level::Moderate),
            vec![Purge::Speculative]
        );
    }

    #[test]
    fn levels_and_purges_are_ordered_by_cost_to_the_user() {
        // `Ord` is load-bearing on both: `Purge::ORDER` is the recovery
        // sequence, and reordering the variants would silently invert it.
        assert!(Level::Normal < Level::Moderate);
        assert!(Level::Moderate < Level::Critical);
        assert!(Level::Critical < Level::Imminent);

        assert!(Purge::Speculative < Purge::Inactive);
        assert!(Purge::Inactive < Purge::Quality);
        assert!(Purge::Quality < Purge::Active);

        let mut sorted = Purge::ORDER;
        sorted.sort();

        assert_eq!(sorted, Purge::ORDER, "ORDER must match the declared order");
    }
}
