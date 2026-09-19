use super::*;

/// Converts some [`UserAttention`] into its `winit` counterpart.
///
/// [`UserAttention`]: window::UserAttention
pub fn user_attention(user_attention: window::UserAttention) -> winit::window::UserAttentionType {
    match user_attention {
        window::UserAttention::Critical => winit::window::UserAttentionType::Critical,
        window::UserAttention::Informational => winit::window::UserAttentionType::Informational,
    }
}

/// Converts some [`window::Direction`] into a [`winit::window::ResizeDirection`].
pub fn resize_direction(resize_direction: window::Direction) -> winit::window::ResizeDirection {
    match resize_direction {
        window::Direction::North => winit::window::ResizeDirection::North,
        window::Direction::South => winit::window::ResizeDirection::South,
        window::Direction::East => winit::window::ResizeDirection::East,
        window::Direction::West => winit::window::ResizeDirection::West,
        window::Direction::NorthEast => winit::window::ResizeDirection::NorthEast,
        window::Direction::NorthWest => winit::window::ResizeDirection::NorthWest,
        window::Direction::SouthEast => winit::window::ResizeDirection::SouthEast,
        window::Direction::SouthWest => winit::window::ResizeDirection::SouthWest,
    }
}

/// Converts some [`window::Icon`] into its `winit` counterpart.
///
/// Returns `None` if there is an error during the conversion.
pub fn icon(icon: window::Icon) -> Option<winit::window::Icon> {
    let (pixels, size) = icon.into_raw();

    winit::window::Icon::from_rgba(pixels, size.width, size.height).ok()
}

/// Converts some [`input_method::Purpose`] into its `winit` counterpart.
pub fn ime_purpose(purpose: input_method::Purpose) -> winit::window::ImePurpose {
    match purpose {
        input_method::Purpose::Normal => winit::window::ImePurpose::Normal,
        input_method::Purpose::Secure => winit::window::ImePurpose::Password,
        input_method::Purpose::Terminal => winit::window::ImePurpose::Terminal,
    }
}
