//! Sliders let users set a value by moving an indicator.
//!
//! # Example
//! ```no_run
//! # mod taim { pub mod widget { pub use taim_widget::*; } pub use taim_widget::Renderer; pub use taim_widget::core::*; }
//! # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
//! #
//! use taim::widget::slider;
//!
//! struct State {
//!    value: f32,
//! }
//!
//! #[derive(Debug, Clone)]
//! enum Message {
//!     ValueChanged(f32),
//! }
//!
//! fn view(state: &State) -> Element<'_, Message> {
//!     slider(0.0..=100.0, state.value, Message::ValueChanged).into()
//! }
//!
//! fn update(state: &mut State, message: Message) {
//!     match message {
//!         Message::ValueChanged(value) => {
//!             state.value = value;
//!         }
//!     }
//! }
//! ```
use std::ops::RangeInclusive;

pub use crate::slider::{Catalog, Handle, HandleShape, Status, Style, StyleFn, default};
use crate::slider::{InteractionState as State, advance, normalize_step, snap};

use crate::core::border::Border;
use crate::core::keyboard;
use crate::core::keyboard::key::{self, Key};
use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::renderer;
use crate::core::touch;
use crate::core::widget::tree::{self, Tree};
use crate::core::window;
use crate::core::{self, Element, Event, Length, Pixels, Point, Rectangle, Shell, Size, Widget};

mod control;
mod implementation;

pub use control::VerticalSlider;
