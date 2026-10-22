//! Small window-management vocabulary between TAIM and `winit`.
use super::*;

/// Translates how insistently the window asks for the user's attention.
pub fn user_attention(attention: window::UserAttention) -> winit::window::UserAttentionType {
    use winit::window::UserAttentionType as Urgency;

    if attention.is_critical() {
        Urgency::Critical
    } else {
        Urgency::Informational
    }
}

/// Translates the edge or corner of an interactive resize.
pub fn resize_direction(direction: window::Direction) -> winit::window::ResizeDirection {
    use window::Direction as D;
    use winit::window::ResizeDirection as Edge;

    match direction {
        D::North => Edge::North,
        D::South => Edge::South,
        D::East => Edge::East,
        D::West => Edge::West,
        D::NorthEast => Edge::NorthEast,
        D::NorthWest => Edge::NorthWest,
        D::SouthEast => Edge::SouthEast,
        D::SouthWest => Edge::SouthWest,
    }
}

/// Builds a platform window icon, or `None` if the pixels are rejected.
pub fn icon(icon: window::Icon) -> Option<winit::window::Icon> {
    let (rgba, size) = icon.into_raw();

    winit::window::Icon::from_rgba(rgba, size.width, size.height).ok()
}

/// Translates what kind of text an input method is composing.
pub fn ime_purpose(purpose: input_method::Purpose) -> winit::window::ImePurpose {
    use winit::window::ImePurpose as Ime;

    match purpose {
        input_method::Purpose::Normal => Ime::Normal,
        // TAIM calls it "secure"; platforms call it "password".
        input_method::Purpose::Secure => Ime::Password,
        input_method::Purpose::Terminal => Ime::Terminal,
    }
}
