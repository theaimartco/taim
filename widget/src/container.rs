//! Containers let you align a widget inside their boundaries.
//!
//! # Example
//! ```no_run
//! # mod taim { pub mod widget { pub use taim_widget::*; } }
//! # pub type State = ();
//! # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
//! use taim::widget::container;
//!
//! enum Message {
//!     // ...
//! }
//!
//! fn view(state: &State) -> Element<'_, Message> {
//!     container("This text is centered inside a rounded box!")
//!         .padding(10)
//!         .center(800)
//!         .style(container::rounded_box)
//!         .into()
//! }
//! ```
use crate::core::alignment::{self, Alignment};
use crate::core::border::{self, Border};
use crate::core::gradient::{self, Gradient};
use crate::core::layout;
use crate::core::material::Material;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::semantics;
use crate::core::theme;
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::visit;
use crate::core::widget::{self, Operation};
use crate::core::{
    self, Background, Color, Element, Event, Layout, Length, Padding, Rectangle, Shadow, Shell,
    Size, Theme, Vector, Widget, color,
};

mod control;
mod implementation;
mod layout_ops;
mod style;

pub use control::Container;
pub use layout_ops::{draw_background, layout};
pub use style::{
    Catalog, Style, StyleFn, background, bordered_box, danger, dark, primary, rounded_box,
    secondary, success, transparent, warning,
};
