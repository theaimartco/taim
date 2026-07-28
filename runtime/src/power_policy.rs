//! Battery, thermal, and visibility, turned into a quality decision.
//!
//! # What this adds over [`quality_controller`]
//!
//! [`quality_controller::Pressure`] already carries the signals the *GPU*
//! produces: how much budget is used, how many frames were missed, how hot
//! it is. Those describe the machine's ability to keep up.
//!
//! They say nothing about whether it is worth keeping up. A device at 4%
//! battery, or one whose window is fully occluded, has no GPU pressure at
//! all — the frames are cheap and on time — and should still be doing far
//! less work. Left to [`quality_controller`] alone, such a device happily
//! renders `Ultra` into a surface nobody can see, on a battery about to
//! die.
//!
//! [`Policy`] supplies the missing half: it converts [`Power`] into extra
//! [`quality_controller::Pressure`], so both kinds of signal arrive at one
//! controller with one hysteresis rule. That composition is the point.
//! Two independent controllers fighting over the same tier is how a
//! device ends up oscillating between `Ultra` and `Efficient` forever,
//! each controller undoing the other, with neither one wrong.
//!
//! # Visibility is not a dimmer
//!
//! [`Visibility::Hidden`] does not reduce quality — it stops rendering.
//! A surface nobody can see does not need a cheaper frame, it needs no
//! frame, and treating "invisible" as "very low quality" would still cost
//! a full pipeline per vsync to produce pixels that are discarded.
//!
//! [`quality_controller`]: crate::quality_controller
//! [`quality_controller::Pressure`]: crate::quality_controller::Pressure

use crate::quality_controller::Pressure;

/// Whether the surface is being looked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Visibility {
    /// On screen.
    #[default]
    Visible,

    /// Partly covered, or on an inactive workspace. Still visible, so
    /// still rendered — a thumbnail or a peeking edge is real content.
    Occluded,

    /// Not composited at all: minimised, on another workspace, screen off.
    Hidden,
}

/// How the device is powered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Supply {
    /// Plugged in. Energy is not the constraint.
    #[default]
    External,

    /// On battery.
    Battery,
}

/// What the platform knows about power.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Power {
    /// Where the energy comes from.
    pub supply: Supply,

    /// Remaining charge, `0.0..=1.0`.
    pub charge: f32,

    /// Whether the user asked for battery saving explicitly.
    ///
    /// Honoured whatever the charge is: a user at 80% who turned this on
    /// has told us something we cannot infer, and second-guessing it
    /// because the battery looks fine is how a setting becomes a lie.
    pub saver: bool,

    /// Thermal pressure, `0.0` (cool) to `1.0` (critical).
    pub thermal: f32,

    /// Whether the surface is being looked at.
    pub visibility: Visibility,
}

impl Default for Power {
    fn default() -> Self {
        Self {
            supply: Supply::External,
            charge: 1.0,
            saver: false,
            thermal: 0.0,
            visibility: Visibility::Visible,
        }
    }
}

impl Power {
    /// Plugged in, cool, visible, full.
    pub const NOMINAL: Self = Self {
        supply: Supply::External,
        charge: 1.0,
        saver: false,
        thermal: 0.0,
        visibility: Visibility::Visible,
    };
}

/// Below this charge, a device on battery starts conserving.
///
/// 20% rather than something lower because the point is to extend the
/// time the user has to *do something about it*. A policy that waited for
/// 5% would save a minute or two and be worth nothing.
pub const CONSERVE_BELOW: f32 = 0.20;

/// Below this charge, conserving becomes the priority.
pub const CRITICAL_BELOW: f32 = 0.10;

/// Turns [`Power`] into the pressure a [`quality_controller`] understands.
///
/// [`quality_controller`]: crate::quality_controller
#[derive(Debug, Clone, Copy, Default)]
pub struct Policy {
    power: Power,
}

impl Policy {
    /// A policy at [`Power::NOMINAL`].
    pub fn new() -> Self {
        Self {
            power: Power::NOMINAL,
        }
    }

    /// Reports what the platform knows.
    pub fn update(&mut self, power: Power) {
        self.power = power;
    }

    /// What the platform last reported.
    pub fn power(&self) -> Power {
        self.power
    }

    /// Whether frames should be produced at all.
    ///
    /// `false` only for [`Visibility::Hidden`]. An occluded surface is
    /// still rendered: a thumbnail or a peeking edge is real content, and
    /// a compositor that asked for a frame will get one.
    pub fn should_render(&self) -> bool {
        self.power.visibility != Visibility::Hidden
    }

    /// How much of the frame budget to aim for, `0.0..=1.0`.
    ///
    /// A separate knob from the quality tier, because they fail
    /// differently: dropping the tier makes each frame cheaper, and
    /// stretching the budget makes frames *less frequent*. A conserving
    /// device wants both, and an occluded one wants only the second — its
    /// frames are already cheap, there are just no eyes on them.
    pub fn frame_budget_scale(&self) -> f32 {
        if !self.should_render() {
            return 0.0;
        }

        let mut scale: f32 = 1.0;

        if self.power.visibility == Visibility::Occluded {
            // Half rate: still correct, a quarter of the energy.
            scale *= 0.5;
        }

        if self.is_conserving() {
            scale *= 0.5;
        }

        scale
    }

    /// Whether the device is trying to save energy.
    pub fn is_conserving(&self) -> bool {
        // An explicit request outranks every inference. A user at 80% who
        // turned this on has told us something we cannot work out, and
        // overriding them because the battery looks fine is how a setting
        // becomes a lie.
        if self.power.saver {
            return true;
        }

        self.power.supply == Supply::Battery && self.power.charge < CONSERVE_BELOW
    }

    /// The [`Pressure`] this power state adds to whatever the GPU
    /// reported.
    ///
    /// Merged into one [`Pressure`] rather than driving a second
    /// controller: two controllers fighting over one tier is how a device
    /// oscillates forever, with neither of them wrong.
    ///
    /// [`Pressure`]: crate::quality_controller::Pressure
    pub fn pressure(&self, gpu: Pressure) -> Pressure {
        Pressure {
            gpu_ratio: gpu.gpu_ratio,
            missed_frame_ratio: gpu.missed_frame_ratio,
            // The larger of the two: the platform's thermal reading and
            // the GPU's are the same physical fact seen from two places,
            // and taking the max means neither source can hide the other's
            // alarm. Adding them would double-count it into a false
            // emergency.
            thermal_ratio: gpu.thermal_ratio.max(self.power.thermal),
            energy_ratio: gpu.energy_ratio,
            recovering: gpu.recovering,
        }
        .with_energy(self.energy_ratio())
    }

    /// How much energy pressure this state implies, `0.0..=1.0`.
    fn energy_ratio(&self) -> f32 {
        if self.power.supply == Supply::External && !self.power.saver {
            // Plugged in, no explicit request: energy is not a constraint.
            // Thermal still is, and travels separately.
            return 0.0;
        }

        if self.power.saver {
            return 1.0;
        }

        if self.power.charge < CRITICAL_BELOW {
            return 1.0;
        }

        if self.power.charge < CONSERVE_BELOW {
            // Ramp from 0 at the conserve threshold to 1 at critical, so
            // quality degrades gradually as the battery drains rather than
            // falling off a cliff the user notices as a glitch.
            let span = CONSERVE_BELOW - CRITICAL_BELOW;
            let into = CONSERVE_BELOW - self.power.charge;

            return (into / span).clamp(0.0, 1.0);
        }

        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on_battery(charge: f32) -> Power {
        Power {
            supply: Supply::Battery,
            charge,
            ..Power::NOMINAL
        }
    }

    #[test]
    fn a_plugged_in_device_is_under_no_energy_pressure() {
        let mut policy = Policy::new();
        policy.update(Power::NOMINAL);

        assert!(!policy.is_conserving());
        assert_eq!(policy.frame_budget_scale(), 1.0);
    }

    #[test]
    fn a_healthy_battery_is_not_a_reason_to_do_less() {
        // Conserving from the moment a cable is unplugged would make every
        // laptop worse for no benefit.
        let mut policy = Policy::new();
        policy.update(on_battery(0.85));

        assert!(!policy.is_conserving());
    }

    #[test]
    fn a_low_battery_conserves() {
        let mut policy = Policy::new();
        policy.update(on_battery(0.15));

        assert!(policy.is_conserving());
    }

    #[test]
    fn an_explicit_request_outranks_a_healthy_battery() {
        // A user at 80% who turned this on has told us something we cannot
        // infer. Second-guessing them is how a setting becomes a lie.
        let mut policy = Policy::new();
        policy.update(Power {
            saver: true,
            ..Power::NOMINAL
        });

        assert!(
            policy.is_conserving(),
            "a plugged-in device at full charge must still honour an explicit request"
        );
        assert_eq!(policy.pressure(Pressure::NOMINAL).energy_ratio(), 1.0);
    }

    #[test]
    fn a_hidden_surface_is_not_rendered_at_all() {
        // Not "rendered cheaply": a surface nobody can see needs no frame,
        // and treating invisible as very-low-quality still costs a full
        // pipeline per vsync to produce pixels that are discarded.
        let mut policy = Policy::new();
        policy.update(Power {
            visibility: Visibility::Hidden,
            ..Power::NOMINAL
        });

        assert!(!policy.should_render());
        assert_eq!(policy.frame_budget_scale(), 0.0);
    }

    #[test]
    fn an_occluded_surface_is_still_rendered_but_less_often() {
        // A thumbnail or a peeking edge is real content. Its frames are
        // already cheap; there are just fewer eyes on them.
        let mut policy = Policy::new();
        policy.update(Power {
            visibility: Visibility::Occluded,
            ..Power::NOMINAL
        });

        assert!(policy.should_render());
        assert_eq!(policy.frame_budget_scale(), 0.5);
    }

    #[test]
    fn conserving_and_occlusion_compound() {
        let mut policy = Policy::new();
        policy.update(Power {
            visibility: Visibility::Occluded,
            ..on_battery(0.05)
        });

        assert_eq!(policy.frame_budget_scale(), 0.25);
    }

    #[test]
    fn energy_pressure_ramps_rather_than_falling_off_a_cliff() {
        // A step change in quality is something the user notices as a
        // glitch. A ramp is something they do not notice at all.
        let mut policy = Policy::new();

        let ratio = |policy: &mut Policy, charge: f32| {
            policy.update(on_battery(charge));
            policy.pressure(Pressure::NOMINAL).energy_ratio()
        };

        assert_eq!(ratio(&mut policy, 0.25), 0.0, "above the threshold");
        assert_eq!(ratio(&mut policy, 0.20), 0.0, "at the threshold");

        let midway = ratio(&mut policy, 0.15);
        assert!(
            (0.4..=0.6).contains(&midway),
            "halfway between conserve and critical must be about half: {midway}"
        );

        assert_eq!(ratio(&mut policy, 0.10), 1.0, "at critical");
        assert_eq!(ratio(&mut policy, 0.02), 1.0, "and stays there");
    }

    #[test]
    fn the_hotter_of_two_thermal_readings_wins() {
        // The platform's reading and the GPU's are the same physical fact
        // seen from two places. Taking the max means neither can hide the
        // other's alarm; adding them would double-count it into a false
        // emergency.
        let mut policy = Policy::new();
        policy.update(Power {
            thermal: 0.9,
            ..Power::NOMINAL
        });

        let merged = policy.pressure(Pressure {
            thermal_ratio: 0.3,
            ..Pressure::NOMINAL
        });

        assert_eq!(merged.thermal_ratio, 0.9);
        assert!(
            merged.thermal_ratio <= 1.0,
            "two hot readings must not add into a false emergency"
        );
    }

    #[test]
    fn a_platform_thermal_alarm_reaches_the_controller_as_an_emergency() {
        // The whole reason to merge into one `Pressure`: the controller's
        // emergency bypass has to see the platform's signal too.
        let mut policy = Policy::new();
        policy.update(Power {
            thermal: 0.99,
            ..Power::NOMINAL
        });

        assert!(policy.pressure(Pressure::NOMINAL).is_emergency());
    }

    #[test]
    fn gpu_signals_pass_through_untouched() {
        // The policy adds; it must not edit what the GPU reported.
        let policy = Policy::new();

        let gpu = Pressure {
            gpu_ratio: 0.7,
            missed_frame_ratio: 0.4,
            thermal_ratio: 0.2,
            energy_ratio: 0.0,
            recovering: true,
        };

        let merged = policy.pressure(gpu);

        assert_eq!(merged.gpu_ratio, gpu.gpu_ratio);
        assert_eq!(merged.missed_frame_ratio, gpu.missed_frame_ratio);
        assert!(merged.recovering);
    }
}
