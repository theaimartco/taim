//! Checkboxes can be used to let users make binary choices.
//!
//! # Example
//! ```no_run
//! # mod taim { pub mod widget { pub use taim_widget::*; } pub use taim_widget::Renderer; pub use taim_widget::core::*; }
//! # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
//! #
//! use taim::widget::checkbox;
//!
//! struct State {
//!    is_checked: bool,
//! }
//!
//! enum Message {
//!     CheckboxToggled(bool),
//! }
//!
//! fn view(state: &State) -> Element<'_, Message> {
//!     checkbox(state.is_checked)
//!         .label("Toggle me!")
//!         .on_toggle(Message::CheckboxToggled)
//!         .into()
//! }
//!
//! fn update(state: &mut State, message: Message) {
//!     match message {
//!         Message::CheckboxToggled(is_checked) => {
//!             state.is_checked = is_checked;
//!         }
//!     }
//! }
//! ```
use crate::core::alignment;
use crate::core::layout;
use crate::core::mouse;
use crate::core::renderer;
use crate::core::text;
use crate::core::theme::palette;
use crate::core::touch;
use crate::core::widget;
use crate::core::widget::tree::{self, Tree};
use crate::core::window;
use crate::core::{
    Background, Border, Color, Element, Event, Layout, Length, Pixels, Rectangle, Shell, Size,
    Theme, Widget,
};

mod control;
mod implementation;
mod style;

pub use control::Checkbox;
pub use style::{Catalog, Icon, Status, Style, StyleFn, danger, primary, secondary, success};
