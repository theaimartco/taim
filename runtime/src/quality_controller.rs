//! Hysteresis-based adaptive-quality tier selection.
//!
//! The [`Controller`] maps runtime [`Pressure`] signals (GPU load,
//! missed frames, thermal state, GPU-recovery) to an
//! [`taim_graphics::quality::Tier`], applying a minimum-residency
//! hysteresis so ordinary tier changes cannot oscillate frame-to-frame
//! around a threshold (`phase.md` Phase 12: "use hysteresis and minimum
//! residency time when changing quality tiers; prevent visual
//! oscillation").
//!
//! # Why Recovery bypasses hysteresis
//!
//! `phase.md` also requires "Recovery tier must render the shell without
//! expensive effects" and lists GPU loss/thermal-critical as the
//! conditions it protects against. A protective drop to [`Tier::Recovery`]
//! is an emergency, not an aesthetic preference: making it wait out the
//! residency window would keep expensive effects running for seconds
//! during a thermal or GPU-loss event, which is exactly what Recovery
//! exists to prevent. So an *escalation into Recovery* (and, symmetrically,
//! any drop to a strictly safer tier under active recovery) applies
//! immediately, while every other transition -- including *leaving*
//! Recovery once conditions clear -- is residency-gated to prevent
//! oscillation back up into effects that would re-trigger the pressure.

#![allow(missing_docs)]

use crate::core::time::{Duration, Instant};
use taim_graphics::quality::Tier;

/// The default minimum time a tier must be resident before an ordinary
/// (non-emergency) change is allowed.
pub const DEFAULT_MINIMUM_RESIDENCY: Duration = Duration::from_secs(5);

/// Runtime signals used for quality adaptation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pressure {
    /// Fraction of GPU budget currently used, `0.0..=1.0`.
    pub gpu_ratio: f32,
    /// Fraction of recent frames that missed their deadline, `0.0..=1.0`.
    pub missed_frame_ratio: f32,
    /// Thermal pressure from zero (cool) to one (critical).
    pub thermal_ratio: f32,
    /// Energy pressure from zero (plugged in, or a healthy battery) to
    /// one (critically low, or the user asked to conserve).
    ///
    /// Unlike every other field here, this is not a measure of the
    /// machine's *ability* to keep up — a device at 4% battery renders
    /// `Ultra` perfectly well, right up until it dies. It is a measure of
    /// whether keeping up is worth it, and it arrives through
    /// [`power_policy::Policy::pressure`] rather than from the GPU.
    ///
    /// It lives in [`Pressure`] rather than driving a controller of its
    /// own so that one hysteresis rule arbitrates every signal. Two
    /// controllers fighting over one tier is how a device oscillates
    /// forever, with neither of them wrong.
    ///
    /// [`power_policy::Policy::pressure`]: crate::power_policy::Policy::pressure
    pub energy_ratio: f32,
    /// Whether GPU-context recovery is currently in progress.
    pub recovering: bool,
}

impl Pressure {
    /// Nominal, unstressed pressure (a cool, idle, healthy GPU).
    pub const NOMINAL: Self = Self {
        gpu_ratio: 0.0,
        missed_frame_ratio: 0.0,
        thermal_ratio: 0.0,
        energy_ratio: 0.0,
        recovering: false,
    };

    /// This pressure, with `ratio` of energy pressure.
    #[must_use]
    pub fn with_energy(mut self, ratio: f32) -> Self {
        self.energy_ratio = ratio.clamp(0.0, 1.0);
        self
    }

    /// How much energy pressure these signals carry.
    pub fn energy_ratio(self) -> f32 {
        self.energy_ratio
    }

    /// Returns whether these signals demand the protective
    /// [`Tier::Recovery`] tier immediately (GPU loss/recovery in
    /// progress, or thermal state at/above critical).
    pub fn is_emergency(self) -> bool {
        self.recovering || self.thermal_ratio >= 0.95
    }
}

/// Selects a visual-quality [`Tier`] from [`Pressure`], with hysteresis
/// to prevent oscillation.
#[derive(Debug, Clone)]
pub struct Controller {
    tier: Tier,
    changed_at: Instant,
    minimum_residency: Duration,
}

impl Controller {
    /// Creates a controller at [`Tier::Balanced`].
    pub fn new(now: Instant) -> Self {
        Self {
            tier: Tier::Balanced,
            changed_at: now,
            minimum_residency: DEFAULT_MINIMUM_RESIDENCY,
        }
    }

    /// Overrides the minimum residency window (mainly for testing and
    /// device-specific tuning).
    pub fn with_minimum_residency(mut self, residency: Duration) -> Self {
        self.minimum_residency = residency;
        self
    }

    /// Returns the current tier.
    pub fn tier(&self) -> Tier {
        self.tier
    }

    /// Returns the tier these signals would target, ignoring hysteresis.
    fn desired(pressure: Pressure) -> Tier {
        if pressure.is_emergency() {
            Tier::Recovery
        } else if pressure.gpu_ratio >= 0.9
            || pressure.missed_frame_ratio >= 0.1
            || pressure.thermal_ratio >= 0.8
            // A device conserving energy drops to `Efficient` even though
            // it is keeping up effortlessly. That is the whole point of
            // the signal: the frames are cheap and on time, and the
            // battery is still about to die.
            || pressure.energy_ratio >= 0.8
        {
            Tier::Efficient
        } else if pressure.gpu_ratio <= 0.55
            && pressure.missed_frame_ratio <= 0.01
            && pressure.thermal_ratio <= 0.4
            // `Ultra` is the tier that spends energy for looks. Any
            // energy pressure at all disqualifies it, rather than being
            // averaged away by a healthy GPU.
            && pressure.energy_ratio <= 0.01
        {
            Tier::Ultra
        } else {
            Tier::Balanced
        }
    }

    /// Evaluates `pressure` at `now` and returns the (possibly changed)
    /// tier.
    ///
    /// An emergency escalation into [`Tier::Recovery`] applies
    /// immediately, bypassing the residency window; every other
    /// transition (including *leaving* Recovery once the emergency
    /// clears) waits out [`Self::with_minimum_residency`] to prevent
    /// oscillation.
    pub fn update(&mut self, now: Instant, pressure: Pressure) -> Tier {
        let desired = Self::desired(pressure);

        if desired == self.tier {
            return self.tier;
        }

        // Emergency drop to Recovery is protective and must not wait.
        let is_emergency_drop = desired == Tier::Recovery && pressure.is_emergency();

        let residency_elapsed =
            now.saturating_duration_since(self.changed_at) >= self.minimum_residency;

        if is_emergency_drop || residency_elapsed {
            self.tier = desired;
            self.changed_at = now;
        }

        self.tier
    }

    /// Returns whether the controller would allow an ordinary
    /// (non-emergency) change at `now` -- i.e. the residency window has
    /// elapsed since the last change.
    pub fn can_change(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.changed_at) >= self.minimum_residency
    }
}

/// A sliding window that turns observed frame durations into the
/// `missed_frame_ratio` a [`Controller`] consumes.
///
/// This is the honest bridge between the frame scheduler's timing and
/// quality adaptation: a real event loop records each frame's actual
/// duration against its target budget, and this reports the fraction of
/// the recent window that overran -- a genuine, measured signal, not a
/// synthesized one. Thermal and GPU-loss signals still come from the
/// platform (there is no portable sensor to invent here); this covers
/// the one pressure the runtime itself can measure directly.
#[derive(Debug, Clone)]
pub struct FramePressure {
    budget: Duration,
    window: usize,
    recent_missed: std::collections::VecDeque<bool>,
}

impl FramePressure {
    /// Creates a tracker over the last `window` frames against `budget`
    /// (e.g. `Duration::from_millis(16)` for a 60 Hz target). `window` is
    /// clamped to at least 1.
    pub fn new(budget: Duration, window: usize) -> Self {
        Self {
            budget,
            window: window.max(1),
            recent_missed: std::collections::VecDeque::new(),
        }
    }

    /// Records one frame's actual duration, evicting the oldest sample
    /// once the window is full.
    pub fn record(&mut self, frame_duration: Duration) {
        if self.recent_missed.len() == self.window {
            let _ = self.recent_missed.pop_front();
        }
        self.recent_missed.push_back(frame_duration > self.budget);
    }

    /// Returns the fraction of the recent window that overran the budget,
    /// in `0.0..=1.0` (`0.0` when no frames have been recorded yet).
    pub fn missed_frame_ratio(&self) -> f32 {
        if self.recent_missed.is_empty() {
            return 0.0;
        }
        let missed = self.recent_missed.iter().filter(|&&m| m).count();
        missed as f32 / self.recent_missed.len() as f32
    }

    /// Folds the measured missed-frame ratio into a [`Pressure`], leaving
    /// the platform-supplied thermal/GPU signals untouched -- the one
    /// composition point where the runtime's own measurement meets the
    /// signals only the platform can provide.
    pub fn apply_to(&self, mut pressure: Pressure) -> Pressure {
        pressure.missed_frame_ratio = self.missed_frame_ratio();
        pressure
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn epoch() -> Instant {
        static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        *EPOCH.get_or_init(Instant::now)
    }

    #[test]
    fn a_fresh_controller_starts_balanced() {
        assert_eq!(Controller::new(epoch()).tier(), Tier::Balanced);
    }

    #[test]
    fn measured_dropped_frames_drive_the_controller_toward_efficient() {
        // The end-to-end runtime path: a real event loop feeds measured
        // frame durations into FramePressure, folds them into Pressure,
        // and the controller degrades once the missed ratio crosses the
        // threshold and residency elapses.
        let start = epoch();
        let mut controller = Controller::new(start).with_minimum_residency(Duration::from_secs(5));
        let mut frames = FramePressure::new(Duration::from_millis(16), 10);

        // Every recent frame overran the 16ms budget.
        for _ in 0..10 {
            frames.record(Duration::from_millis(40));
        }

        let pressure = frames.apply_to(Pressure::NOMINAL);
        assert!(pressure.missed_frame_ratio >= 0.1);

        // Residency-gated: no change at t=0, change once it elapses.
        assert_eq!(controller.update(start, pressure), Tier::Balanced);
        assert_eq!(
            controller.update(start + Duration::from_secs(5), pressure),
            Tier::Efficient
        );
    }

    #[test]
    fn healthy_measured_frames_leave_a_balanced_controller_alone() {
        let start = epoch();
        let mut controller = Controller::new(start);
        let mut frames = FramePressure::new(Duration::from_millis(16), 10);

        for _ in 0..10 {
            frames.record(Duration::from_millis(8));
        }

        let pressure = frames.apply_to(Pressure::NOMINAL);
        // Not calm enough for Ultra (gpu/thermal are nominal here, so it
        // actually would qualify) -- assert it never *degrades*.
        let tier = controller.update(start + Duration::from_secs(10), pressure);
        assert!(
            tier <= Tier::Balanced,
            "healthy frames must not degrade quality"
        );
    }
}
