//! Window configuration.

pub mod icon;

pub use icon::Icon;

pub use crate::core::window::{Level, Position, Settings};

/// Settings for a window of `desired` size, kept within `minimum` and
/// `maximum`.
///
/// Invalid or contradictory sizes are clamped rather than handed to the
/// platform.
pub fn constrained_settings(
    desired: crate::core::Size,
    minimum: Option<crate::core::Size>,
    maximum: Option<crate::core::Size>,
) -> Settings {
    Settings::default()
        .with_size(desired)
        .with_size_limits(minimum, maximum)
        .normalized()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_initial_size_does_not_reach_native_window() {
        let size = crate::core::Size::new(f32::NAN, 12.0);
        let configured =
            constrained_settings(size, Some(crate::core::Size::new(400.0, 300.0)), None);
        assert!(configured.size.width.is_finite());
        assert!(configured.size.width >= 400.0);
        assert!(configured.size.height >= 300.0);
    }
}
