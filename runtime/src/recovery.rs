//! What to do when the renderer dies, and what to do when it keeps dying.
//!
//! # The ladder
//!
//! `phase.md`'s recovery sequence ends where [`memory_pressure`] leaves
//! off:
//!
//! 4. Rebuild lost GPU resources by generation.
//! 5. Restart the renderer while preserving safe application state.
//! 6. Escalate to a minimal Recovery-tier shell if repeated restart fails.
//!
//! Each rung is more disruptive and less likely to be needed. The
//! interesting engineering is not in any one of them — it is in the two
//! rules that stop the ladder from becoming a trap.
//!
//! # Rule one: generations, not flags
//!
//! > Never continue with potentially stale GPU handles.
//!
//! A `bool` saying "the device was lost" cannot express *which* resources
//! are stale, and the failure is silent: a buffer handle from the dead
//! device is a number, and using it is undefined rather than an error. So
//! every resource carries the [`Generation`] it was made in, and
//! [`Recovery::is_current`] is the only way to ask whether it is safe.
//! A resource from a previous generation is not "probably fine" — it is
//! unusable, and the type says so.
//!
//! # Rule two: a crash loop is worse than a crash
//!
//! Restarting a renderer that immediately dies again is not recovery; it
//! is a loop that burns the battery, floods the log, and leaves the user
//! looking at a flickering screen that never settles. It is *worse* than
//! stopping, because stopping at least presents something.
//!
//! So restarts are counted within a window ([`Limit`]), and exceeding it
//! escalates to [`Stage::Safe`] — a minimal shell that can still render a
//! lock screen and essential settings. A user locked out of their own
//! device by a graphics driver bug should still be able to see the time
//! and turn the thing off.
//!
//! [`memory_pressure`]: crate::memory_pressure

use crate::core::time::{Duration, Instant};

/// Which era of GPU resources is current.
///
/// Bumped every time the device is lost and recreated. A resource made in
/// generation `n` is unusable in generation `n + 1`, and the only way to
/// know is to have kept the number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Generation(u64);

impl Generation {
    /// The first generation.
    pub const FIRST: Self = Self(0);

    /// The generation after this one.
    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }

    /// This generation as a number, for logging.
    pub fn get(self) -> u64 {
        self.0
    }
}

/// Why the renderer stopped.
///
/// Classified rather than lumped together, because the right response
/// differs: a device that was removed is not coming back on its own, and
/// retrying an out-of-memory failure without freeing anything just fails
/// again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fault {
    /// The GPU device was lost — a driver reset, a TDR, a suspend that
    /// tore the context down. Recreating usually works.
    DeviceLost,

    /// A GPU operation timed out. The device may still be wedged, so this
    /// is treated as a loss rather than retried in place.
    Timeout,

    /// A GPU allocation failed. Recreating without freeing anything first
    /// will fail identically, so this is the one fault that *requires*
    /// [`memory_pressure`](crate::memory_pressure) to act before a
    /// restart is worth attempting.
    OutOfMemory,

    /// The adapter was physically removed — an eGPU unplugged, a
    /// hot-swapped display. Nothing to recreate on.
    DeviceRemoved,
}

impl Fault {
    /// Whether recreating the device could plausibly help.
    ///
    /// `false` for [`Fault::DeviceRemoved`]: there is no device. Looping
    /// on a recreate that cannot succeed is the crash loop this module
    /// exists to prevent, and this is the case where it is knowable up
    /// front rather than after five attempts.
    pub fn is_recoverable(self) -> bool {
        !matches!(self, Fault::DeviceRemoved)
    }

    /// Whether memory must be freed before a restart is worth attempting.
    pub fn demands_purge(self) -> bool {
        matches!(self, Fault::OutOfMemory)
    }
}

/// How many restarts are allowed, and over what window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limit {
    /// How many restarts may occur within [`Limit::window`].
    pub restarts: u32,
    /// The window restarts are counted over.
    pub window: Duration,
}

impl Default for Limit {
    /// Three restarts a minute.
    ///
    /// Chosen so that a genuine, isolated driver reset — which happens,
    /// and recovers — never trips it, while a real loop trips it in a few
    /// seconds rather than after minutes of flickering. A window rather
    /// than a total, because a device that failed once an hour ago and
    /// once now is not looping; it is a device with an occasional fault,
    /// and dropping such a user into a minimal shell forever would be its
    /// own bug.
    fn default() -> Self {
        Self {
            restarts: 3,
            window: Duration::from_secs(60),
        }
    }
}

/// What the shell should be doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Stage {
    /// Everything is fine.
    #[default]
    Running,

    /// The device is being recreated and resources rebuilt. Frames are
    /// not being produced.
    Restarting,

    /// Restarting kept failing. A minimal shell — lock screen, essential
    /// settings — on the most conservative quality tier.
    ///
    /// Not a crash. A user locked out by a driver bug should still be able
    /// to see the time and turn the device off.
    Safe,
}

/// What a caller should do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Free memory first; this fault will recur otherwise.
    Purge,

    /// Recreate the device and rebuild resources for the new generation.
    Restart(Generation),

    /// Give up on the full shell and present the minimal one.
    Escalate,
}

/// Tracks faults, decides restarts, and refuses to loop.
#[derive(Debug, Clone)]
pub struct Recovery {
    generation: Generation,
    stage: Stage,
    limit: Limit,
    /// When each restart in the current window happened.
    restarts: Vec<Instant>,
}

impl Recovery {
    /// A healthy [`Recovery`] at [`Generation::FIRST`].
    pub fn new() -> Self {
        Self {
            generation: Generation::FIRST,
            stage: Stage::Running,
            limit: Limit::default(),
            restarts: Vec::new(),
        }
    }

    /// Sets the crash-loop limit.
    #[must_use]
    pub fn with_limit(mut self, limit: Limit) -> Self {
        self.limit = limit;
        self
    }

    /// The current generation.
    pub fn generation(&self) -> Generation {
        self.generation
    }

    /// What the shell should be doing.
    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// Whether a resource made in `generation` is safe to use.
    ///
    /// The only way to ask. A resource from a previous generation is not
    /// "probably fine" — its handle refers to a device that no longer
    /// exists, and using it is undefined rather than an error.
    pub fn is_current(&self, generation: Generation) -> bool {
        generation == self.generation && self.stage != Stage::Restarting
    }

    /// Reports a `fault`, returning what to do about it.
    ///
    /// Bumps the generation for anything recoverable, so every resource
    /// made before this moment stops being current *immediately* — before
    /// the caller has done anything, and whether or not the restart
    /// eventually succeeds. A generation bumped only on success would
    /// leave a window where stale handles still answered `is_current`.
    pub fn fault(&mut self, now: Instant, fault: Fault) -> Action {
        // A device that was physically removed cannot be recreated.
        // Looping on it is knowable up front, rather than after five
        // attempts.
        if !fault.is_recoverable() {
            self.stage = Stage::Safe;
            self.generation = self.generation.next();

            return Action::Escalate;
        }

        self.forget_old_restarts(now);

        if self.restarts.len() as u32 >= self.limit.restarts {
            self.stage = Stage::Safe;
            self.generation = self.generation.next();

            return Action::Escalate;
        }

        // Invalidate everything from the dead device before returning:
        // the caller is about to go and do things, and a stale handle
        // answering `is_current` in the meantime is the exact failure this
        // module exists to prevent.
        self.generation = self.generation.next();
        self.stage = Stage::Restarting;
        self.restarts.push(now);

        if fault.demands_purge() {
            // Recreating without freeing anything will fail identically.
            return Action::Purge;
        }

        Action::Restart(self.generation)
    }

    /// Reports that a restart succeeded.
    ///
    /// Does **not** clear the restart history: three successful restarts
    /// in a minute is still a loop, and a device that recovers just long
    /// enough to fail again would otherwise never trip the limit. That is
    /// the shape of a real driver bug, not a hypothetical one.
    pub fn restarted(&mut self) {
        if self.stage == Stage::Restarting {
            self.stage = Stage::Running;
        }
    }

    /// Whether the shell is confined to the minimal, safe presentation.
    pub fn is_safe(&self) -> bool {
        self.stage == Stage::Safe
    }

    /// Leaves [`Stage::Safe`], for a caller that has reason to believe the
    /// underlying problem is gone — the user plugged the eGPU back in, or
    /// picked a different adapter.
    ///
    /// Clears the restart history, because this is a statement that
    /// something *changed*. Nothing calls this automatically: a timer that
    /// left the safe shell on its own would rediscover the same broken
    /// driver and start the loop again, which is precisely what the limit
    /// was for.
    pub fn recover(&mut self) {
        self.stage = Stage::Running;
        self.generation = self.generation.next();
        self.restarts.clear();
    }

    /// How many restarts have happened in the current window.
    pub fn recent_restarts(&self) -> usize {
        self.restarts.len()
    }

    fn forget_old_restarts(&mut self, now: Instant) {
        let window = self.limit.window;

        self.restarts
            .retain(|restart| now.duration_since(*restart) < window);
    }
}

impl Default for Recovery {
    fn default() -> Self {
        Self::new()
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
    fn a_healthy_recovery_says_its_resources_are_current() {
        let recovery = Recovery::new();

        assert!(recovery.is_current(Generation::FIRST));
        assert_eq!(recovery.stage(), Stage::Running);
    }

    #[test]
    fn a_lost_device_invalidates_every_resource_immediately() {
        // The rule with no exceptions: never continue with potentially
        // stale GPU handles. The generation bumps on the fault, not on the
        // successful restart -- a bump on success would leave a window
        // where a handle to a dead device still answered `is_current`.
        let mut recovery = Recovery::new();
        let old = recovery.generation();

        let _ = recovery.fault(at(0), Fault::DeviceLost);

        assert!(
            !recovery.is_current(old),
            "a handle from the dead device must stop being current at once"
        );
    }

    #[test]
    fn nothing_is_current_while_restarting() {
        // Not even the new generation: the device does not exist yet.
        let mut recovery = Recovery::new();

        let Action::Restart(new) = recovery.fault(at(0), Fault::DeviceLost) else {
            panic!("a lost device must ask for a restart");
        };

        assert_eq!(recovery.stage(), Stage::Restarting);
        assert!(!recovery.is_current(new), "the new device is not built yet");

        recovery.restarted();

        assert!(recovery.is_current(new));
    }

    #[test]
    fn an_out_of_memory_fault_demands_a_purge_before_a_restart() {
        // Recreating without freeing anything will fail identically.
        let mut recovery = Recovery::new();

        assert_eq!(recovery.fault(at(0), Fault::OutOfMemory), Action::Purge);
    }

    #[test]
    fn a_removed_device_escalates_without_trying() {
        // There is no device to recreate. Looping on it is knowable up
        // front rather than after five attempts.
        let mut recovery = Recovery::new();

        assert_eq!(
            recovery.fault(at(0), Fault::DeviceRemoved),
            Action::Escalate
        );
        assert!(recovery.is_safe());
        assert_eq!(
            recovery.recent_restarts(),
            0,
            "an impossible restart must not be counted as an attempt"
        );
    }

    #[test]
    fn a_removed_device_still_invalidates_its_resources() {
        // Escalating is not an excuse to keep stale handles: the safe
        // shell has to draw too, and it must not draw with them.
        let mut recovery = Recovery::new();
        let old = recovery.generation();

        let _ = recovery.fault(at(0), Fault::DeviceRemoved);

        assert!(!recovery.is_current(old));
    }

    #[test]
    fn a_crash_loop_escalates_to_the_safe_shell() {
        // Restarting a renderer that immediately dies again is worse than
        // stopping: it burns battery, floods the log, and leaves the user
        // watching a screen that never settles.
        let mut recovery = Recovery::new().with_limit(Limit {
            restarts: 3,
            window: Duration::from_secs(60),
        });

        for millis in [0, 100, 200] {
            assert!(matches!(
                recovery.fault(at(millis), Fault::DeviceLost),
                Action::Restart(_)
            ));
            recovery.restarted();
        }

        assert_eq!(
            recovery.fault(at(300), Fault::DeviceLost),
            Action::Escalate,
            "the fourth failure in a minute is a loop, not an accident"
        );
        assert!(recovery.is_safe());
    }

    #[test]
    fn a_successful_restart_does_not_forgive_the_loop() {
        // A device that recovers just long enough to fail again is the
        // shape of a real driver bug. Clearing the history on success
        // would mean it never trips the limit.
        let mut recovery = Recovery::new().with_limit(Limit {
            restarts: 2,
            window: Duration::from_secs(60),
        });

        let _ = recovery.fault(at(0), Fault::DeviceLost);
        recovery.restarted();
        let _ = recovery.fault(at(100), Fault::DeviceLost);
        recovery.restarted();

        assert_eq!(recovery.fault(at(200), Fault::DeviceLost), Action::Escalate);
    }

    #[test]
    fn an_occasional_fault_is_not_a_loop() {
        // A window, not a total: a device that failed once an hour ago and
        // once now has an occasional fault, and dropping that user into a
        // minimal shell forever would be its own bug.
        let mut recovery = Recovery::new().with_limit(Limit {
            restarts: 2,
            window: Duration::from_millis(500),
        });

        let _ = recovery.fault(at(0), Fault::DeviceLost);
        recovery.restarted();
        let _ = recovery.fault(at(100), Fault::DeviceLost);
        recovery.restarted();

        // Well past the window: the earlier failures are forgotten.
        assert!(matches!(
            recovery.fault(at(5_000), Fault::DeviceLost),
            Action::Restart(_)
        ));
        assert!(!recovery.is_safe());
    }

    #[test]
    fn the_safe_shell_is_left_only_when_something_actually_changed() {
        // Nothing calls `recover` automatically. A timer that left the
        // safe shell on its own would rediscover the same broken driver
        // and start the loop again -- which is what the limit was for.
        let mut recovery = Recovery::new().with_limit(Limit {
            restarts: 1,
            window: Duration::from_secs(60),
        });

        let _ = recovery.fault(at(0), Fault::DeviceLost);
        recovery.restarted();
        let _ = recovery.fault(at(100), Fault::DeviceLost);

        assert!(recovery.is_safe());

        recovery.recover();

        assert_eq!(recovery.stage(), Stage::Running);
        assert_eq!(recovery.recent_restarts(), 0);
        assert!(
            matches!(
                recovery.fault(at(200), Fault::DeviceLost),
                Action::Restart(_),
            ),
            "after a real change, the budget starts again"
        );
    }

    #[test]
    fn recovering_invalidates_resources_too() {
        let mut recovery = Recovery::new();
        let _ = recovery.fault(at(0), Fault::DeviceRemoved);

        let stale = recovery.generation();
        recovery.recover();

        assert!(!recovery.is_current(stale));
    }

    #[test]
    fn generations_never_repeat() {
        // A wrapped or reused generation would make a stale handle look
        // current again, which is the one thing this type exists to
        // prevent.
        let mut recovery = Recovery::new().with_limit(Limit {
            restarts: 100,
            window: Duration::from_secs(60),
        });

        let mut seen = vec![recovery.generation()];

        for millis in 0..20 {
            let _ = recovery.fault(at(millis), Fault::DeviceLost);
            recovery.restarted();

            assert!(
                !seen.contains(&recovery.generation()),
                "generation {:?} was reused",
                recovery.generation()
            );

            seen.push(recovery.generation());
        }
    }

    #[test]
    fn faults_are_classified_by_what_helps() {
        assert!(Fault::DeviceLost.is_recoverable());
        assert!(Fault::Timeout.is_recoverable());
        assert!(Fault::OutOfMemory.is_recoverable());
        assert!(!Fault::DeviceRemoved.is_recoverable());

        assert!(Fault::OutOfMemory.demands_purge());
        assert!(!Fault::DeviceLost.demands_purge());
    }
}
