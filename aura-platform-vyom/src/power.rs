//! Visibility, suspend, battery, and thermal signals.

/// Compositor thermal-pressure level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Thermal {
    /// No pressure.
    Nominal,
    /// Mild pressure.
    Fair,
    /// Sustained serious pressure.
    Serious,
    /// Emergency degradation required.
    Critical,
}

/// Latest platform power and visibility snapshot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerState {
    /// Surface is visible.
    pub visible: bool,
    /// Session is suspended.
    pub suspended: bool,
    /// Device is drawing from battery.
    pub on_battery: bool,
    /// Normalized charge when known.
    pub battery_fraction: Option<f32>,
    /// Thermal pressure.
    pub thermal: Thermal,
}

impl Default for PowerState {
    fn default() -> Self {
        Self {
            visible: true,
            suspended: false,
            on_battery: false,
            battery_fraction: None,
            thermal: Thermal::Nominal,
        }
    }
}

impl PowerState {
    /// Clamps untrusted platform fractions.
    pub fn sanitize(mut self) -> Self {
        self.battery_fraction = self.battery_fraction.map(|value| value.clamp(0.0, 1.0));
        self
    }

    /// Returns whether frame production is permitted.
    pub const fn should_render(&self) -> bool {
        self.visible && !self.suspended
    }
}
