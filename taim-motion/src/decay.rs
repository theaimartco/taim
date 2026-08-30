//! Fling decay with bounded handoff information.

/// Optional inclusive bounds for a decaying value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    /// Inclusive lower bound.
    pub min: f32,
    /// Inclusive upper bound.
    pub max: f32,
}

impl Bounds {
    /// Creates bounds, normalizing reversed inputs.
    pub fn new(min: f32, max: f32) -> Self {
        Self {
            min: min.min(max),
            max: min.max(max),
        }
    }
}

/// The outcome of one decay advance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecayStep {
    /// New value after the elapsed interval.
    pub value: f32,
    /// Remaining velocity after the elapsed interval.
    pub velocity: f32,
    /// The bound reached, when the decay should hand off to a parent or spring.
    pub hit_bound: Option<f32>,
}

/// Exponentially decaying scalar motion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Decay {
    value: f32,
    velocity: f32,
    friction: f32,
    bounds: Option<Bounds>,
    epsilon: f32,
}

impl Decay {
    /// Creates an unbounded decay. `friction` is the per-second exponential rate.
    pub fn new(value: f32, velocity: f32, friction: f32) -> Self {
        Self {
            value,
            velocity,
            friction: friction.max(0.001),
            bounds: None,
            epsilon: 0.01,
        }
    }

    /// Restricts the value to inclusive bounds.
    pub fn bounds(mut self, bounds: Bounds) -> Self {
        self.bounds = Some(bounds);
        self
    }

    /// Advances using the exact integral of exponential velocity decay.
    pub fn advance(&mut self, seconds: f32) -> DecayStep {
        let seconds = seconds.max(0.0);
        let decay = libm::expf(-self.friction * seconds);
        let next_velocity = self.velocity * decay;
        let delta = self.velocity * (1.0 - decay) / self.friction;
        let mut next_value = self.value + delta;
        let mut hit_bound = None;

        if let Some(bounds) = self.bounds
            && (next_value < bounds.min || next_value > bounds.max)
        {
            let bound = if next_value < bounds.min {
                bounds.min
            } else {
                bounds.max
            };
            next_value = bound;
            hit_bound = Some(bound);
        }

        self.value = next_value;
        self.velocity = if hit_bound.is_some() || next_velocity.abs() <= self.epsilon {
            0.0
        } else {
            next_velocity
        };

        DecayStep {
            value: self.value,
            velocity: self.velocity,
            hit_bound,
        }
    }

    /// Returns whether the decay is settled.
    pub fn is_at_rest(&self) -> bool {
        self.velocity == 0.0
    }
}
