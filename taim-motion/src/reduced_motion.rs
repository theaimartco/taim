//! Reduced-motion policy mapping.

/// The semantic visual effect requested by a transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Spatial movement or transform.
    Transform,
    /// Opacity change.
    Fade,
    /// A state change with no visual interpolation.
    Instant,
}

/// User motion preference applied per effect, not as a global timer switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MotionPolicy {
    /// Preserve authored effects and duration.
    #[default]
    Full,
    /// Replace spatial motion with short fades while retaining meaningful state cues.
    Reduced,
}

impl MotionPolicy {
    /// Maps an authored effect to its accessible semantic alternative.
    pub const fn effect(self, effect: Effect) -> Effect {
        match (self, effect) {
            (Self::Reduced, Effect::Transform) => Effect::Fade,
            _ => effect,
        }
    }

    /// Maps duration for the effect while preserving a visible cue for fades.
    pub fn duration(self, effect: Effect, authored_seconds: f64) -> f64 {
        let authored_seconds = authored_seconds.max(0.0);
        match (self, self.effect(effect)) {
            (_, Effect::Instant) => 0.0,
            (Self::Full, _) => authored_seconds,
            (Self::Reduced, Effect::Fade) => authored_seconds.min(0.15),
            (Self::Reduced, Effect::Transform) => 0.0,
        }
    }
}
