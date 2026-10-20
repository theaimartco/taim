//! Listen and react to touch events.
pub use crate::core::touch::{Event, Finger};

/// Map raw platform touch events into TAIM's checked unified pointer model.
///
/// Invalid active coordinates are rejected; malformed terminal coordinates
/// become a safe cancellation so gesture captures cannot remain stuck.
#[inline]
pub fn checked_pointer(
    event: &Event,
    timestamp: crate::core::time::Instant,
) -> Option<crate::core::pointer::Pointer> {
    event.to_pointer(timestamp).sanitize_coordinates()
}

#[cfg(test)]
mod taim_touch_adapter_tests {
    use super::*;

    #[test]
    fn malformed_terminal_touch_becomes_safe_cancel() {
        let event = Event::FingerLifted {
            id: Finger(19),
            position: crate::core::Point::new(f32::NAN, 0.0),
            metadata: crate::core::pointer::Metadata::NONE,
        };
        let checked = checked_pointer(&event, crate::core::time::Instant::now()).unwrap();
        assert_eq!(checked.phase, crate::core::pointer::Phase::Cancel);
        assert_eq!(checked.position, crate::core::Point::ORIGIN);
    }
}
