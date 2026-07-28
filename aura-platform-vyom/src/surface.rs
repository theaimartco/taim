//! Surface lifecycle and configure/ack invariants.

use std::num::NonZeroU64;

/// Stable surface identity. Zero is never a valid protocol identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SurfaceId(NonZeroU64);

impl SurfaceId {
    /// Creates an identity, rejecting the reserved zero value.
    pub fn new(raw: u64) -> Option<Self> {
        NonZeroU64::new(raw).map(Self)
    }

    /// Returns the non-zero numeric identity.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Observable state of a Wayland surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceState {
    /// No buffer may be attached until the first configure is acknowledged.
    AwaitingConfigure,
    /// The latest acknowledged configuration.
    Configured {
        /// Configure serial.
        serial: u32,
        /// Logical width.
        width: u32,
        /// Logical height.
        height: u32,
    },
    /// Surface production is paused.
    Suspended,
    /// Surface is terminal and cannot be reused.
    Closed,
}

/// Protocol state machine for one surface.
#[derive(Debug, Clone)]
pub struct SurfaceLifecycle {
    id: SurfaceId,
    state: SurfaceState,
    scale_120: u32,
    last_acked_serial: Option<u32>,
}

impl SurfaceLifecycle {
    /// Starts a surface in the pre-configure state.
    pub fn new(id: SurfaceId) -> Self {
        Self {
            id,
            state: SurfaceState::AwaitingConfigure,
            scale_120: 120,
            last_acked_serial: None,
        }
    }

    /// Returns the stable identity.
    pub const fn id(&self) -> SurfaceId {
        self.id
    }

    /// Returns the current lifecycle state.
    pub const fn state(&self) -> SurfaceState {
        self.state
    }

    /// Acknowledges and applies a compositor configuration.
    pub fn configure(&mut self, serial: u32, width: u32, height: u32) -> bool {
        if matches!(self.state, SurfaceState::Closed) {
            return false;
        }

        let width = width.max(1);
        let height = height.max(1);
        self.last_acked_serial = Some(serial);
        self.state = SurfaceState::Configured {
            serial,
            width,
            height,
        };
        true
    }

    /// Pauses rendering without destroying the identity.
    pub fn suspend(&mut self) {
        if !matches!(self.state, SurfaceState::Closed) {
            self.state = SurfaceState::Suspended;
        }
    }

    /// Requests a new configure after suspension.
    pub fn resume(&mut self) {
        if matches!(self.state, SurfaceState::Suspended) {
            self.state = SurfaceState::AwaitingConfigure;
        }
    }

    /// Permanently closes the surface.
    pub fn close(&mut self) {
        self.state = SurfaceState::Closed;
    }

    /// Fractional scale in 120ths, as defined by the Wayland protocol.
    pub fn set_scale_120(&mut self, scale_120: u32) -> bool {
        if scale_120 == 0 || matches!(self.state, SurfaceState::Closed) {
            return false;
        }
        self.scale_120 = scale_120;
        true
    }

    /// Returns the scale as a floating-point multiplier.
    pub fn scale_factor(&self) -> f64 {
        f64::from(self.scale_120) / 120.0
    }

    /// Returns the last configure serial acknowledged by the client.
    pub const fn last_acked_serial(&self) -> Option<u32> {
        self.last_acked_serial
    }

    /// Returns whether attaching a new buffer is protocol-correct.
    pub const fn may_attach_buffer(&self) -> bool {
        matches!(self.state, SurfaceState::Configured { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id() -> SurfaceId {
        SurfaceId::new(1).unwrap()
    }

    #[test]
    fn a_buffer_cannot_be_attached_before_configure() {
        let lifecycle = SurfaceLifecycle::new(id());
        assert!(!lifecycle.may_attach_buffer());
    }

    #[test]
    fn configure_is_acknowledged_and_zero_size_is_sanitized() {
        let mut lifecycle = SurfaceLifecycle::new(id());
        assert!(lifecycle.configure(42, 0, 0));
        assert_eq!(lifecycle.last_acked_serial(), Some(42));
        assert_eq!(
            lifecycle.state(),
            SurfaceState::Configured {
                serial: 42,
                width: 1,
                height: 1
            }
        );
    }

    #[test]
    fn a_closed_surface_never_resurrects() {
        let mut lifecycle = SurfaceLifecycle::new(id());
        lifecycle.close();
        lifecycle.resume();
        assert!(!lifecycle.configure(1, 10, 10));
        assert_eq!(lifecycle.state(), SurfaceState::Closed);
    }
}
