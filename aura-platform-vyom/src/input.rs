//! Seat and device lifecycle.

use std::collections::{BTreeMap, BTreeSet};

use aura_core::Point;
use aura_core::pointer::{DeviceKind, Id, Metadata, Phase, Pointer};
use aura_core::time::Instant;

/// Compositor seat identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SeatId(pub u32);

/// Physical input-device identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(pub u64);

/// Device registered under a seat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Device {
    /// Stable device identity.
    pub id: DeviceId,
    /// Normalized device class.
    pub kind: DeviceKind,
}

#[derive(Debug, Default)]
struct Seat {
    devices: BTreeMap<DeviceId, Device>,
    contacts: BTreeSet<Id>,
}

/// Multi-seat input registry. Removing a seat or device always emits
/// cancellation for every contact that can no longer complete normally.
#[derive(Debug, Default)]
pub struct Input {
    seats: BTreeMap<SeatId, Seat>,
}

impl Input {
    /// Adds or replaces a device under a seat.
    pub fn add_device(&mut self, seat: SeatId, device: Device) {
        let _ = self
            .seats
            .entry(seat)
            .or_default()
            .devices
            .insert(device.id, device);
    }

    /// Removes a device and cancels contacts it can no longer complete.
    pub fn remove_device(&mut self, seat: SeatId, device: DeviceId, at: Instant) -> Vec<Pointer> {
        let Some(seat) = self.seats.get_mut(&seat) else {
            return Vec::new();
        };
        let Some(device) = seat.devices.remove(&device) else {
            return Vec::new();
        };

        seat.contacts
            .iter()
            .copied()
            .map(|id| Pointer {
                id,
                device: device.kind,
                phase: Phase::Cancel,
                position: Point::ORIGIN,
                timestamp: at,
                metadata: Metadata::NONE,
                captured: false,
            })
            .collect()
    }

    /// Removes a seat and cancels every active contact.
    pub fn remove_seat(&mut self, id: SeatId, at: Instant) -> Vec<Pointer> {
        let Some(seat) = self.seats.remove(&id) else {
            return Vec::new();
        };

        seat.contacts
            .into_iter()
            .map(|id| Pointer {
                id,
                device: DeviceKind::Touch,
                phase: Phase::Cancel,
                position: Point::ORIGIN,
                timestamp: at,
                metadata: Metadata::NONE,
                captured: false,
            })
            .collect()
    }

    /// Validates one pointer event against contact lifecycle.
    pub fn pointer(&mut self, seat: SeatId, pointer: Pointer) -> Option<Pointer> {
        let seat = self.seats.get_mut(&seat)?;

        match pointer.phase {
            Phase::Down => {
                if !seat.contacts.insert(pointer.id) {
                    return None;
                }
            }
            Phase::Move => {
                if !seat.contacts.contains(&pointer.id) {
                    return None;
                }
            }
            Phase::Up | Phase::Cancel => {
                if !seat.contacts.remove(&pointer.id) {
                    return None;
                }
            }
        }

        Some(pointer)
    }

    /// Returns contacts active across all seats.
    pub fn active_contacts(&self) -> usize {
        self.seats.values().map(|seat| seat.contacts.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hot_unplug_cancels_active_contacts() {
        let now = Instant::now();
        let mut input = Input::default();
        let seat = SeatId(1);
        input.add_device(
            seat,
            Device {
                id: DeviceId(9),
                kind: DeviceKind::Touch,
            },
        );
        let down = Pointer {
            id: Id::from_raw(7),
            device: DeviceKind::Touch,
            phase: Phase::Down,
            position: Point::ORIGIN,
            timestamp: now,
            metadata: Metadata::NONE,
            captured: false,
        };
        assert!(input.pointer(seat, down).is_some());

        let cancelled = input.remove_device(seat, DeviceId(9), now);
        assert_eq!(cancelled.len(), 1);
        assert_eq!(cancelled[0].phase, Phase::Cancel);
    }
}
