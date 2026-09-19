//! Text editors display a multi-line text input for text editing.
//!
//! # Example
//! ```no_run
//! # mod taim { pub mod widget { pub use taim_widget::*; } pub use taim_widget::Renderer; pub use taim_widget::core::*; }
//! # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
//! #
//! use taim::widget::text_editor;
//!
//! struct State {
//!    content: text_editor::Content,
//! }
//!
//! #[derive(Debug, Clone)]
//! enum Message {
//!     Edit(text_editor::Action)
//! }
//!
//! fn view(state: &State) -> Element<'_, Message> {
//!     text_editor(&state.content)
//!         .placeholder("Type something here...")
//!         .on_action(Message::Edit)
//!         .into()
//! }
//!
//! fn update(state: &mut State, message: Message) {
//!     match message {
//!         Message::Edit(action) => {
//!             state.content.perform(action);
//!         }
//!     }
//! }
//! ```
use crate::core::alignment;
use crate::core::clipboard;
use crate::core::input_method;
use crate::core::keyboard;
use crate::core::keyboard::key;
use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::renderer;
use crate::core::text::editor::Editor as _;
use crate::core::text::highlighter::{self, Highlighter};
use crate::core::text::{self, LineHeight, Text, Wrapping};
use crate::core::theme;
use crate::core::time::{Duration, Instant};
use crate::core::touch;
use crate::core::widget::operation;
use crate::core::widget::{self, Widget};
use crate::core::window;
use crate::core::{
    Background, Border, Color, Element, Event, InputMethod, Length, Padding, Pixels, Point,
    Rectangle, Shell, Size, SmolStr, Theme, Vector,
};

use std::borrow::Cow;
use std::cell::RefCell;
use std::fmt;
use std::ops;
use std::ops::DerefMut;
use std::sync::Arc;

pub use text::editor::{Action, Cursor, Edit, Line, LineEnding, Motion, Position, Selection};

mod binding;
mod content;
mod editor;
mod implementation;
mod state;
mod style;

pub use binding::{Binding, KeyPress};
pub use content::Content;
pub use editor::TextEditor;
pub use state::State;
pub use style::{Catalog, Status, Style, StyleFn, default};

#[cfg(target_os = "macos")]
pub(crate) use binding::convert_macos_shortcut;
