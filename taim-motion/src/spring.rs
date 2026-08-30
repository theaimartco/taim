//! Stable damped springs with position and velocity continuity.

/// Parameters for a scalar spring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringConfig {
    /// Natural angular frequency in radians per second. Must be positive.
    pub angular_frequency: f32,
    /// Damping ratio: below one oscillates, one is critical, above one is overdamped.
    pub damping_ratio: f32,
    /// Position and velocity tolerance used to declare rest.
    pub epsilon: f32,
}

impl Default for SpringConfig {
    fn default() -> Self {
        Self {
            angular_frequency: 18.0,
            damping_ratio: 1.0,
            epsilon: 0.001,
        }
    }
}

/// A scalar damped spring.
///
/// Retargeting changes only `target`; the current position and velocity are
/// retained, so gesture release and rapid reverse operations have no jump.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    value: f32,
    velocity: f32,
    target: f32,
    config: SpringConfig,
}

impl Spring {
    /// Creates a spring at rest at `value`.
    pub fn new(value: f32) -> Self {
        Self::with_config(value, SpringConfig::default())
    }

    /// Creates a spring with explicit physical parameters.
    pub fn with_config(value: f32, mut config: SpringConfig) -> Self {
        config.angular_frequency = config.angular_frequency.max(0.001);
        config.damping_ratio = config.damping_ratio.max(0.0);
        config.epsilon = config.epsilon.max(0.000_001);

        Self {
            value,
            velocity: 0.0,
            target: value,
            config,
        }
    }

    /// Returns the current value.
    pub const fn value(&self) -> f32 {
        self.value
    }

    /// Returns the current velocity in units per second.
    pub const fn velocity(&self) -> f32 {
        self.velocity
    }

    /// Sets the target without discarding velocity.
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Replaces the velocity, for example at gesture handoff.
    pub fn set_velocity(&mut self, velocity: f32) {
        self.velocity = velocity;
    }

    /// Advances by elapsed seconds using the analytic damped-spring solution.
    pub fn advance(&mut self, seconds: f32) -> f32 {
        let seconds = seconds.max(0.0);
        if seconds == 0.0 || self.is_at_rest() {
            return self.value;
        }

        let omega = self.config.angular_frequency;
        let zeta = self.config.damping_ratio;
        let displacement = self.value - self.target;
        let velocity = self.velocity;

        let (next_displacement, next_velocity) = if (zeta - 1.0).abs() < 0.0001 {
            let decay = libm::expf(-omega * seconds);
            let coefficient = velocity + omega * displacement;
            let position = (displacement + coefficient * seconds) * decay;
            let speed = (velocity - omega * coefficient * seconds) * decay;
            (position, speed)
        } else if zeta < 1.0 {
            let damped = omega * libm::sqrtf(1.0 - zeta * zeta);
            let decay = libm::expf(-zeta * omega * seconds);
            let cosine = libm::cosf(damped * seconds);
            let sine = libm::sinf(damped * seconds);
            let coefficient = (velocity + zeta * omega * displacement) / damped;
            let position = decay * (displacement * cosine + coefficient * sine);
            let speed = decay
                * (velocity * cosine
                    - ((zeta * omega * velocity + omega * omega * displacement) / damped) * sine);
            (position, speed)
        } else {
            let root = libm::sqrtf(zeta * zeta - 1.0);
            let r1 = -omega * (zeta - root);
            let r2 = -omega * (zeta + root);
            let c1 = (velocity - r2 * displacement) / (r1 - r2);
            let c2 = displacement - c1;
            let a = c1 * libm::expf(r1 * seconds);
            let b = c2 * libm::expf(r2 * seconds);
            (a + b, r1 * a + r2 * b)
        };

        self.value = self.target + next_displacement;
        self.velocity = next_velocity;

        if self.is_at_rest() {
            self.value = self.target;
            self.velocity = 0.0;
        }

        self.value
    }

    /// Returns whether this spring has settled.
    pub fn is_at_rest(&self) -> bool {
        (self.target - self.value).abs() <= self.config.epsilon
            && self.velocity.abs() <= self.config.epsilon
    }
}
