use aura_core::input_method::{Purpose, SurroundingText};
use aura_core::pointer::{DeviceKind, Id, Metadata, Phase, Pointer};
use aura_core::time::Instant;
use aura_core::{Point, Rectangle};
use aura_platform_vyom::ime::Session;
use aura_platform_vyom::input::{Device, DeviceId, Input, SeatId};

#[test]
fn hot_unplug_cancels_each_contact_exactly_once() {
    let now = Instant::now();
    let seat = SeatId(1);
    let device = DeviceId(2);
    let mut input = Input::default();
    input.add_device(
        seat,
        Device {
            id: device,
            kind: DeviceKind::Touch,
        },
    );

    for raw in 1..=5 {
        let event = Pointer {
            id: Id::from_raw(raw),
            device: DeviceKind::Touch,
            phase: Phase::Down,
            position: Point::ORIGIN,
            timestamp: now,
            metadata: Metadata::NONE,
            captured: false,
        };
        assert!(input.pointer(seat, event).is_some());
    }

    let cancelled = input.remove_device(seat, device, now);
    assert_eq!(cancelled.len(), 5);
    assert!(cancelled.iter().all(|event| event.phase == Phase::Cancel));
    assert!(input.remove_device(seat, device, now).is_empty());
}

#[test]
fn secure_ime_never_exposes_surrounding_text() {
    let visible = SurroundingText::Visible {
        content: "correct horse battery staple".into(),
        selection: 0..28,
    };
    let session = Session::new(Purpose::Secure, visible, Rectangle::default());
    assert!(session.surrounding_text().is_redacted());
}
