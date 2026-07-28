//! Draw and interact with text.
mod rich;

pub use crate::core::text::{Fragment, Highlighter, IntoFragment, Span};
pub use crate::core::widget::text::*;
pub use rich::Rich;

/// A bunch of text.
///
/// # Example
/// ```no_run
/// # mod aura { pub mod widget { pub use aura_widget::*; } pub use aura_widget::Renderer; pub use aura_widget::core::*; }
/// # pub type State = ();
/// # pub type Element<'a, Message> = aura_widget::core::Element<'a, Message, aura_widget::Theme, aura_widget::Renderer>;
/// use aura::widget::text;
/// use aura::color;
///
/// enum Message {
///     // ...
/// }
///
/// fn view(state: &State) -> Element<'_, Message> {
///     text("Hello, this is aura!")
///         .size(20)
///         .color(color!(0x0000ff))
///         .into()
/// }
/// ```
pub type Text<'a, Theme = crate::Theme, Renderer = crate::Renderer> =
    crate::core::widget::Text<'a, Theme, Renderer>;
