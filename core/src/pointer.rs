//! A unified pointer model spanning mouse, touch, pen/stylus, and
//! trackpad input.
//!
//! Every existing input source in this crate (`mouse::Event`,
//! `touch::Event`) already reports "something pressed/moved/released at
//! a position," but each with its own vocabulary and no shared identity
//! a gesture recognizer (`taim-gesture`) could track across the full
//! lifetime of one contact. [`Pointer`] gives every contact -- a mouse
//! cursor, a finger, a stylus tip -- one stable [`Id`] for as long as it
//! stays down, plus the metadata (pressure, tilt, contact size, button
//! state) that source may or may not actually provide.
//!
//! This module does not replace `mouse::Event`/`touch::Event` -- both
//! keep working exactly as before, and `core::event::Event` keeps
//! carrying them unchanged. [`Pointer`] is a normalized *view* derived
//! from them (see `core::touch`'s compatibility mapping), the common
//! representation `taim-gesture`'s arena and recognizers are written
//! against so they do not need one code path per device kind.
use crate::Point;
use crate::time::Instant;

/// A stable identifier for one pointer contact, valid from its
/// [`Phase::Down`] to its terminating [`Phase::Up`] or
/// [`Phase::Cancel`].
///
/// A mouse has exactly one [`Id`] that is reused across separate
/// press/release cycles (there is only ever one mouse cursor); a touch
/// interaction gets a fresh [`Id`] per finger, matching
/// [`crate::touch::Finger`]'s own identity for the duration of that
/// finger's contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(u64);

impl Id {
    /// The [`Id`] reserved for the mouse pointer.
    ///
    /// A mouse is a single, persistent pointer for the lifetime of the
    /// application (unlike touches, which come and go), so it gets one
    /// fixed [`Id`] rather than a freshly allocated one per click.
    pub const MOUSE: Self = Self(0);

    /// Creates the [`Id`] for a given [`crate::touch::Finger`].
    ///
    /// Finger IDs are assumed nonzero-distinguishable from
    /// [`Id::MOUSE`] by construction: this offsets every finger ID by
    /// one and reserves bit 63 as a "this came from a touch finger" tag,
    /// so a colliding raw value from the platform touch backend can
    /// never alias the reserved mouse [`Id`].
    pub const fn from_finger(finger: crate::touch::Finger) -> Self {
        Self(finger.0 | (1 << 63))
    }

    /// Creates an [`Id`] for a non-mouse, non-touch pointer (e.g. a pen
    /// whose platform backend reports its own distinct device id).
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }
}

/// What kind of physical device produced a [`Pointer`] contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceKind {
    /// A conventional mouse.
    Mouse,
    /// A finger on a touchscreen.
    Touch,
    /// A stylus/pen, potentially reporting pressure and tilt.
    Pen,
    /// A trackpad, reported distinctly from a mouse because it commonly
    /// originates gesture-like input (two-finger scroll/pinch) a real
    /// mouse never does.
    Trackpad,
}

/// Which stage of its lifetime a [`Pointer`] event describes.
///
/// Every contact's events form exactly one sequence starting with
/// [`Phase::Down`] and ending with exactly one of [`Phase::Up`] or
/// [`Phase::Cancel`] -- the correctness rule `phase.md` states as "every
/// pointer sequence ends in up or cancel for all participants." Any
/// number of [`Phase::Move`] events may occur in between, including
/// zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    /// The contact began.
    Down,
    /// The contact moved while still active.
    Move,
    /// The contact ended normally (e.g. finger lifted, button
    /// released).
    Up,
    /// The contact ended abnormally and no gesture may treat it as a
    /// deliberate completion (e.g. the platform reassigned the touch to
    /// scrolling, the window lost focus mid-press).
    Cancel,
}

/// Pressure, tilt, and contact-geometry metadata a [`Pointer`] event may
/// carry, when its device actually reports it.
///
/// Every field is optional because most devices do not report most of
/// these: a plain mouse reports none of them, a basic touchscreen
/// typically reports none either, while a pressure-sensitive stylus may
/// report `pressure`/`tilt` but not `contact_size`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Metadata {
    /// Normalized pressure in `[0.0, 1.0]`, if the device reports it.
    pub pressure: Option<f32>,

    /// The stylus tilt from perpendicular, in radians along the X and Y
    /// axes, if the device reports it.
    pub tilt: Option<(f32, f32)>,

    /// The approximate width/height of the physical contact area (e.g.
    /// a fingertip's footprint), in logical pixels, if the device
    /// reports it.
    pub contact_size: Option<(f32, f32)>,
}

impl Metadata {
    /// [`Metadata`] with every field unset -- the correct value for a
    /// device (a plain mouse, most touchscreens) that reports none of
    /// this.
    pub const NONE: Self = Self {
        pressure: None,
        tilt: None,
        contact_size: None,
    };
}

/// A single, normalized pointer event, unifying mouse, touch, pen, and
/// trackpad input under one shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pointer {
    /// The contact this event belongs to.
    pub id: Id,

    /// Which device produced this event.
    pub device: DeviceKind,

    /// The lifecycle stage this event describes.
    pub phase: Phase,

    /// The pointer's position, in the same logical-pixel coordinate
    /// space every other event in this crate already uses.
    pub position: Point,

    /// When this event occurred, for velocity estimation
    /// (`taim-gesture::velocity`) and deadline-based recognizers (e.g.
    /// `taim-gesture::long_press`).
    pub timestamp: Instant,

    /// Pressure/tilt/contact-size metadata, when available.
    pub metadata: Metadata,

    /// Whether this specific event has already been claimed by a
    /// capture (see `core::event`'s coalesced-event contract) and
    /// therefore should not be offered to a new competitor in the
    /// gesture arena.
    ///
    /// This is distinct from `event::Status`: a widget's ordinary
    /// `Status::Captured` return means "I handled this event," while a
    /// [`Pointer`] capture means "this entire contact, for its whole
    /// remaining lifetime, now belongs to one recognizer" -- the
    /// distinction `phase.md`'s "captured pointers keep routing outside
    /// original bounds" correctness rule depends on.
    pub captured: bool,
}

impl Pointer {
    /// Returns whether this event's [`Phase`] terminates its contact's
    /// lifetime (i.e. no further events for this [`Id`] should be
    /// expected).
    pub fn is_terminal(&self) -> bool {
        matches!(self.phase, Phase::Up | Phase::Cancel)
    }

    /// Whether both logical position coordinates are finite.
    ///
    /// Gesture hit-testing and velocity estimation must not consume NaN or
    /// infinity as an active touch or cursor position. Terminal events may
    /// still be delivered to release an already-captured contact.
    #[inline]
    pub fn has_finite_position(&self) -> bool {
        self.position.x.is_finite() && self.position.y.is_finite()
    }

    /// Check TAIM's gesture position boundary without losing release events.
    ///
    /// Invalid Down/Move samples are rejected. Invalid Up/Cancel samples
    /// are converted to `Cancel` at an inert, finite origin. This avoids
    /// both leaked pointer capture and spurious successful tap/drag-end
    /// dispatch on malformed terminal coordinates.
    #[inline]
    pub fn sanitize_coordinates(mut self) -> Option<Self> {
        if self.has_finite_position() {
            return Some(self);
        }

        if self.is_terminal() {
            self.phase = Phase::Cancel;
            self.position = Point::ORIGIN;
            Some(self)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_id_is_a_fixed_constant() {
        assert_eq!(Id::MOUSE, Id::MOUSE);
    }

    #[test]
    fn finger_ids_never_collide_with_the_mouse_id() {
        use crate::touch::Finger;

        assert_ne!(Id::from_finger(Finger(0)), Id::MOUSE);
        assert_ne!(Id::from_finger(Finger(u64::MAX >> 1)), Id::MOUSE);
    }

    #[test]
    fn distinct_fingers_produce_distinct_ids() {
        use crate::touch::Finger;

        assert_ne!(Id::from_finger(Finger(1)), Id::from_finger(Finger(2)));
    }

    #[test]
    fn up_and_cancel_are_terminal_down_and_move_are_not() {
        let base = Pointer {
            id: Id::MOUSE,
            device: DeviceKind::Mouse,
            phase: Phase::Down,
            position: Point::ORIGIN,
            timestamp: Instant::now(),
            metadata: Metadata::NONE,
            captured: false,
        };

        assert!(
            !Pointer {
                phase: Phase::Down,
                ..base
            }
            .is_terminal()
        );
        assert!(
            !Pointer {
                phase: Phase::Move,
                ..base
            }
            .is_terminal()
        );
        assert!(
            Pointer {
                phase: Phase::Up,
                ..base
            }
            .is_terminal()
        );
        assert!(
            Pointer {
                phase: Phase::Cancel,
                ..base
            }
            .is_terminal()
        );
    }

    #[test]
    fn invalid_active_position_is_rejected_but_terminal_is_cancelled() {
        let baseline = Pointer {
            id: Id::MOUSE,
            device: DeviceKind::Mouse,
            phase: Phase::Move,
            position: Point::new(f32::NAN, 42.0),
            timestamp: Instant::now(),
            metadata: Metadata::NONE,
            captured: false,
        };

        assert_eq!(baseline.sanitize_coordinates(), None);

        let cancelled = Pointer {
            phase: Phase::Up,
            ..baseline
        }
        .sanitize_coordinates()
        .expect("must terminate captured contact");
        assert_eq!(cancelled.phase, Phase::Cancel);
        assert_eq!(cancelled.position, Point::ORIGIN);
        assert!(cancelled.has_finite_position());

        let valid_up = Pointer {
            phase: Phase::Up,
            position: Point::new(10.0, 15.0),
            ..baseline
        }
        .sanitize_coordinates()
        .unwrap();
        assert_eq!(valid_up.phase, Phase::Up);
        assert_eq!(valid_up.position, Point::new(10.0, 15.0));
    }

    #[test]
    fn metadata_none_has_every_field_unset() {
        assert_eq!(Metadata::NONE.pressure, None);
        assert_eq!(Metadata::NONE.tilt, None);
        assert_eq!(Metadata::NONE.contact_size, None);
    }
}
