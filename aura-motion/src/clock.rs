//! The adapter between a runtime's monotonic frame clock and motion math.

/// A monotonic timestamp in seconds relative to an arbitrary origin.
///
/// Only differences between timestamps have meaning. Keeping the unit as a
/// small value relative to an application-local origin avoids loss of
/// precision from converting an operating system uptime to `f64`.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Timestamp(f64);

impl Timestamp {
    /// Creates a timestamp measured in seconds from the clock's origin.
    pub const fn from_seconds(seconds: f64) -> Self {
        Self(seconds)
    }

    /// Returns the number of seconds from the clock's origin.
    pub const fn as_seconds(self) -> f64 {
        self.0
    }

    /// Returns a non-negative elapsed duration from `earlier` to `self`.
    pub fn elapsed_since(self, earlier: Self) -> f64 {
        (self.0 - earlier.0).max(0.0)
    }
}

/// Supplies the one shared, monotonic time source used by motion.
///
/// Implementations must not return a decreasing timestamp. The trait is
/// intentionally tiny so a production runtime can wrap its frame context and
/// tests can use a deterministic fake clock.
pub trait Clock {
    /// Returns the current monotonic timestamp.
    fn now(&self) -> Timestamp;
}

impl Clock for Timestamp {
    fn now(&self) -> Timestamp {
        *self
    }
}
