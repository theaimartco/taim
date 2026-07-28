//! What is moving, and how fast.
//!
//! # What the framework will tell us, and what it will not
//!
//! `phase.md` asks for "timelines, velocity, spring and reduced motion".
//! `UserInterface::update` returns exactly one fact about animation:
//! `is_animating` — a `bool`.
//!
//! That is not a shortcoming of this panel. `aura_motion` owns springs,
//! curves and velocity, and a widget that animates holds its own state
//! inside its `Tree`. Neither is reachable from outside: `Visit` carries
//! bounds, children and semantics, and none of those is a velocity.
//!
//! So this panel shows what exists — whether anything is moving, and how
//! long it has been — and says plainly what does not. A timeline drawn
//! from a `bool` would be a picture of an assumption.
//!
//! # Why a bool is still worth a panel
//!
//! Because the expensive bug is not "the spring is wrong". It is
//! **animating forever**: a spring that never settles, or a widget that
//! requests a redraw every frame for something that stopped moving a
//! minute ago. That keeps the GPU awake and the battery draining, and it
//! is invisible — the screen looks still. `is_animating` catches exactly
//! that, and [`Activity`] is built to make it obvious.

use aura::widget::{column, container, text};
use aura::{Element, Length};

/// What this panel cannot show, and why.
pub const MISSING: &str = "Timelines, velocity and spring state are not shown. `UserInterface` \
                           reports one `bool`; springs live in `aura_motion` and per-widget \
                           animation state lives inside each widget's `Tree`, and `Visit` carries \
                           neither. Drawing a timeline from a bool would be a picture of an \
                           assumption.";

/// How long a run of animation may last before it is suspicious.
///
/// Ten seconds. Long enough that no real transition, spring settle, or
/// loading spinner cycle trips it; short enough that a stuck animation is
/// caught while a developer is still looking at it.
pub const SUSPICIOUS_FRAMES: u64 = 600;

/// How long the target has been animating, in frames.
///
/// Counts a *run*, not a total: an application that animated for a second
/// once an hour ago is fine, and a total would slowly accuse it of being
/// stuck.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Activity {
    /// Frames in the current run of animation. Zero when still.
    run: u64,
    /// The longest run seen.
    longest: u64,
}

impl Activity {
    /// Records one frame.
    pub fn record(&mut self, is_animating: bool) {
        if is_animating {
            self.run += 1;
            self.longest = self.longest.max(self.run);
        } else {
            self.run = 0;
        }
    }

    /// Frames in the current run.
    pub fn run(&self) -> u64 {
        self.run
    }

    /// The longest run seen.
    pub fn longest(&self) -> u64 {
        self.longest
    }

    /// Whether something has been moving long enough to be a bug.
    ///
    /// The expensive one: a spring that never settles keeps the GPU awake
    /// and the battery draining while the screen looks perfectly still.
    pub fn is_suspicious(&self) -> bool {
        self.run >= SUSPICIOUS_FRAMES
    }
}

/// Renders the animation inspector.
pub fn view<'a, Message: 'a>(activity: Activity, is_animating: bool) -> Element<'a, Message> {
    let headline = if is_animating { "Animating" } else { "Still" };

    let verdict = if activity.is_suspicious() {
        format!(
            "STUCK? {} frames without settling. A spring that never settles keeps the GPU awake \
             while the screen looks still.",
            activity.run()
        )
    } else if is_animating {
        format!("{} frames in this run", activity.run())
    } else {
        format!("longest run: {} frames", activity.longest())
    };

    container(
        column![
            text(headline).size(20),
            text(verdict).size(13),
            text(MISSING).size(11),
        ]
        .spacing(8),
    )
    .center_x(Length::Fill)
    .padding(12)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_still_target_has_no_run() {
        let mut activity = Activity::default();

        activity.record(false);

        assert_eq!(activity.run(), 0);
        assert!(!activity.is_suspicious());
    }

    #[test]
    fn a_run_counts_consecutive_frames() {
        let mut activity = Activity::default();

        for _ in 0..5 {
            activity.record(true);
        }

        assert_eq!(activity.run(), 5);
    }

    #[test]
    fn settling_resets_the_run() {
        // A run, not a total: an application that animated for a second an
        // hour ago is fine, and a total would slowly accuse it of being
        // stuck.
        let mut activity = Activity::default();

        for _ in 0..5 {
            activity.record(true);
        }

        activity.record(false);

        assert_eq!(activity.run(), 0);
        assert_eq!(activity.longest(), 5, "but the longest run is remembered");
    }

    #[test]
    fn a_long_run_is_suspicious() {
        // The expensive bug: animating forever, invisibly.
        let mut activity = Activity::default();

        for _ in 0..SUSPICIOUS_FRAMES {
            activity.record(true);
        }

        assert!(activity.is_suspicious());
    }

    #[test]
    fn an_ordinary_transition_is_not_suspicious() {
        // A warning that fires on every animation is a warning nobody
        // reads. Ten seconds is far longer than any real transition.
        let mut activity = Activity::default();

        for _ in 0..120 {
            activity.record(true);
        }

        assert!(!activity.is_suspicious());
    }

    #[test]
    fn settling_clears_suspicion() {
        let mut activity = Activity::default();

        for _ in 0..SUSPICIOUS_FRAMES {
            activity.record(true);
        }

        activity.record(false);

        assert!(
            !activity.is_suspicious(),
            "a settled animation is not stuck, however long it ran"
        );
    }

    #[test]
    fn the_longest_run_survives_a_shorter_one() {
        let mut activity = Activity::default();

        for _ in 0..10 {
            activity.record(true);
        }
        activity.record(false);
        for _ in 0..3 {
            activity.record(true);
        }

        assert_eq!(activity.longest(), 10);
    }

    #[test]
    fn the_panel_says_it_has_only_a_bool() {
        // Drawing a timeline from a bool would be a picture of an
        // assumption.
        assert!(MISSING.contains("bool"), "{MISSING}");
        assert!(MISSING.contains("Visit"), "{MISSING}");
    }
}
