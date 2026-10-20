//! TAIM geometry command validation before allocating backend frame state.
use crate::core::Rectangle;

/// Why a requested geometry frame cannot be presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundsError {
    /// Layout geometry contains NaN or infinity.
    NonFinite,
    /// A coordinate extent is negative.
    NegativeSize,
}

/// Validate a frame's coordinate space without allocating renderer storage.
pub fn validate_frame_bounds(bounds: Rectangle) -> Result<(), BoundsError> {
    let components = [bounds.x, bounds.y, bounds.width, bounds.height];
    if !components.iter().all(|value| value.is_finite()) {
        return Err(BoundsError::NonFinite);
    }
    if bounds.width < 0.0 || bounds.height < 0.0 {
        return Err(BoundsError::NegativeSize);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Point, Size};

    #[test]
    fn rejects_invalid_backend_geometry_without_allocation() {
        assert!(validate_frame_bounds(Rectangle::new(
            Point::ORIGIN,
            Size::new(100.0, 100.0),
        )).is_ok());
        assert_eq!(
            validate_frame_bounds(Rectangle::new(
                Point::new(f32::NAN, 1.0),
                Size::new(1.0, 1.0),
            )),
            Err(BoundsError::NonFinite)
        );
        assert_eq!(
            validate_frame_bounds(Rectangle::new(
                Point::ORIGIN,
                Size::new(-1.0, 2.0),
            )),
            Err(BoundsError::NegativeSize)
        );
    }
}
