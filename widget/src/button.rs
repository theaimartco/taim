//! Buttons allow your users to perform actions by pressing them.
//!
//! # Example
//! ```no_run
//! # mod taim { pub mod widget { pub use taim_widget::*; } }
//! # pub type State = ();
//! # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
//! use taim::widget::button;
//!
//! #[derive(Clone)]
//! enum Message {
//!     ButtonPressed,
//! }
//!
//! fn view(state: &State) -> Element<'_, Message> {
//!     button("Press me!").on_press(Message::ButtonPressed).into()
//! }
//! ```
use crate::core::border::{self, Border};
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::theme::palette;
use crate::core::touch;
use crate::core::widget::Operation;
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::visit;
use crate::core::window;
use crate::core::{
    Background, Color, Element, Event, Layout, Length, Padding, Rectangle, Shadow, Shell, Size,
    Theme, Vector, Widget,
};

mod control;
mod implementation;
mod style;

pub use control::Button;
pub use style::{
    Catalog, DEFAULT_PADDING, Status, Style, StyleFn, background, danger, primary, secondary,
    subtle, success, text, warning,
};
