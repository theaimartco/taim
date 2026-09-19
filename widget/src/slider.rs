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
use crate::core::border::{self, Border};
use crate::core::keyboard;
use crate::core::keyboard::key::{self, Key};
use crate::core::layout;
use crate::core::mouse;
use crate::core::renderer;
use crate::core::touch;
use crate::core::widget::tree::{self, Tree};
use crate::core::window;
use crate::core::{
    self, Background, Color, Element, Event, Layout, Length, Pixels, Point, Rectangle, Shell, Size,
    Theme, Widget,
};

use std::ops::RangeInclusive;

mod control;
mod implementation;
mod math;
mod style;

pub use control::Slider;
pub(crate) use implementation::State as InteractionState;
pub(crate) use math::{advance, normalize_step, snap};
pub use style::{Catalog, Handle, HandleShape, Rail, Status, Style, StyleFn, default};
