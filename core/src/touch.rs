//! Build touch events.
use crate::Point;
use crate::pointer;

/// The smallest edge, in logical pixels, an interactive surface may
/// present as a touch target.
///
/// This constant is the single source of the policy for the whole
/// workspace. `taim-design` re-exports it as `MIN_TOUCH_TARGET` and
/// widgets clamp to it directly, so there is exactly one number to
/// change and no way for a widget to drift from the design contract.
///
/// VYOM is touch-first, so the floor is 48px. See
/// [`MIN_TARGET_COMPATIBILITY`] for the explicit opt-out.
pub const MIN_TARGET: f32 = 48.0;

/// The reduced touch-target floor available to UI that explicitly opts
/// out of [`MIN_TARGET`].
///
/// This is the 44px figure common to platform accessibility guidance.
/// It exists so "we deliberately match the host platform here" can be
/// written down, rather than appearing as an unexplained smaller number
/// in a widget.
pub const MIN_TARGET_COMPATIBILITY: f32 = 44.0;

/// A touch interaction.
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(missing_docs)]
pub enum Event {
    /// A touch interaction was started.
    FingerPressed {
        id: Finger,
        position: Point,
        /// Pressure/tilt/contact-size metadata, when the platform
        /// backend reports it. `pointer::Metadata::NONE` on backends
        /// (most of them, today) that report none of this.
        metadata: pointer::Metadata,
    },

    /// An on-going touch interaction was moved.
    FingerMoved {
        id: Finger,
        position: Point,
        metadata: pointer::Metadata,
    },

    /// A touch interaction was ended.
    FingerLifted {
        id: Finger,
        position: Point,
        metadata: pointer::Metadata,
    },

    /// A touch interaction was canceled.
    FingerLost {
        id: Finger,
        position: Point,
        metadata: pointer::Metadata,
    },
}

impl Event {
    /// Returns the [`Finger`] this [`Event`] belongs to.
    pub fn finger(&self) -> Finger {
        match self {
            Event::FingerPressed { id, .. }
            | Event::FingerMoved { id, .. }
            | Event::FingerLifted { id, .. }
            | Event::FingerLost { id, .. } => *id,
        }
    }

    /// Returns the position this [`Event`] occurred at.
    pub fn position(&self) -> Point {
        match self {
            Event::FingerPressed { position, .. }
            | Event::FingerMoved { position, .. }
            | Event::FingerLifted { position, .. }
            | Event::FingerLost { position, .. } => *position,
        }
    }

    /// Returns the [`pointer::Metadata`] carried by this [`Event`].
    pub fn metadata(&self) -> pointer::Metadata {
        match self {
            Event::FingerPressed { metadata, .. }
            | Event::FingerMoved { metadata, .. }
            | Event::FingerLifted { metadata, .. }
            | Event::FingerLost { metadata, .. } => *metadata,
        }
    }

    /// Converts this [`Event`] into a unified [`pointer::Pointer`]
    /// event, given the [`pointer::Instant`] it occurred at.
    ///
    /// This is the compatibility mapping `taim-gesture` (and anything
    /// else that wants one normalized pointer model instead of
    /// device-specific branches) is expected to use: a touch's
    /// [`Finger`] becomes a [`pointer::Id`] stable for that finger's
    /// whole contact (via [`pointer::Id::from_finger`]), and each of
    /// this enum's four variants maps onto exactly one
    /// [`pointer::Phase`], preserving the "every sequence ends in up or
    /// cancel" invariant those phases exist to encode.
    pub fn to_pointer(&self, timestamp: crate::time::Instant) -> pointer::Pointer {
        let phase = match self {
            Event::FingerPressed { .. } => pointer::Phase::Down,
            Event::FingerMoved { .. } => pointer::Phase::Move,
            Event::FingerLifted { .. } => pointer::Phase::Up,
            Event::FingerLost { .. } => pointer::Phase::Cancel,
        };

        pointer::Pointer {
            id: pointer::Id::from_finger(self.finger()),
            device: pointer::DeviceKind::Touch,
            phase,
            position: self.position(),
            timestamp,
            metadata: self.metadata(),
            captured: false,
        }
    }
}

/// A unique identifier representing a finger on a touch interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Finger(pub u64);

#[cfg(test)]
mod tests {
    use super::*;

    fn event(metadata: pointer::Metadata) -> Event {
        Event::FingerPressed {
            id: Finger(7),
            position: Point::new(1.0, 2.0),
            metadata,
        }
    }

    #[test]
    fn finger_extracts_the_right_id_for_every_variant() {
        let position = Point::ORIGIN;
        let metadata = pointer::Metadata::NONE;

        assert_eq!(
            Event::FingerPressed {
                id: Finger(1),
                position,
                metadata
            }
            .finger(),
            Finger(1)
        );
        assert_eq!(
            Event::FingerMoved {
                id: Finger(2),
                position,
                metadata
            }
            .finger(),
            Finger(2)
        );
        assert_eq!(
            Event::FingerLifted {
                id: Finger(3),
                position,
                metadata
            }
            .finger(),
            Finger(3)
        );
        assert_eq!(
            Event::FingerLost {
                id: Finger(4),
                position,
                metadata
            }
            .finger(),
            Finger(4)
        );
    }

    #[test]
    fn position_and_metadata_round_trip() {
        let metadata = pointer::Metadata {
            pressure: Some(0.5),
            tilt: None,
            contact_size: None,
        };

        let event = event(metadata);

        assert_eq!(event.position(), Point::new(1.0, 2.0));
        assert_eq!(event.metadata(), metadata);
    }

    #[test]
    fn to_pointer_maps_each_variant_to_the_right_phase() {
        let now = crate::time::Instant::now();
        let position = Point::ORIGIN;
        let metadata = pointer::Metadata::NONE;

        let pressed = Event::FingerPressed {
            id: Finger(1),
            position,
            metadata,
        }
        .to_pointer(now);
        assert_eq!(pressed.phase, pointer::Phase::Down);

        let moved = Event::FingerMoved {
            id: Finger(1),
            position,
            metadata,
        }
        .to_pointer(now);
        assert_eq!(moved.phase, pointer::Phase::Move);

        let lifted = Event::FingerLifted {
            id: Finger(1),
            position,
            metadata,
        }
        .to_pointer(now);
        assert_eq!(lifted.phase, pointer::Phase::Up);
        assert!(lifted.is_terminal());

        let lost = Event::FingerLost {
            id: Finger(1),
            position,
            metadata,
        }
        .to_pointer(now);
        assert_eq!(lost.phase, pointer::Phase::Cancel);
        assert!(lost.is_terminal());
    }

    #[test]
    fn to_pointer_uses_a_stable_id_derived_from_the_finger() {
        let now = crate::time::Instant::now();
        let position = Point::ORIGIN;
        let metadata = pointer::Metadata::NONE;

        let a = Event::FingerPressed {
            id: Finger(9),
            position,
            metadata,
        }
        .to_pointer(now);
        let b = Event::FingerMoved {
            id: Finger(9),
            position,
            metadata,
        }
        .to_pointer(now);

        assert_eq!(
            a.id, b.id,
            "the same finger must map to the same pointer::Id across events"
        );
        assert_eq!(a.device, pointer::DeviceKind::Touch);
    }
}
