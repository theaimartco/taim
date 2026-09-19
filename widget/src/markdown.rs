//! Markdown widgets can parse and display Markdown.
//!
//! You can enable the `highlighter` feature for syntax highlighting
//! in code blocks.
//!
//! Only the variants of [`Item`] are currently supported.
//!
//! # Example
//! ```no_run
//! # mod taim { pub mod widget { pub use taim_widget::*; } pub use taim_widget::Renderer; pub use taim_widget::core::*; }
//! # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
//! #
//! use taim::widget::markdown;
//! use taim::Theme;
//!
//! struct State {
//!    markdown: Vec<markdown::Item>,
//! }
//!
//! enum Message {
//!     LinkClicked(markdown::Uri),
//! }
//!
//! impl State {
//!     pub fn new() -> Self {
//!         Self {
//!             markdown: markdown::parse("This is some **Markdown**!").collect(),
//!         }
//!     }
//!
//!     fn view(&self) -> Element<'_, Message> {
//!         markdown::view(&self.markdown, Theme::Dark)
//!             .map(Message::LinkClicked)
//!             .into()
//!     }
//!
//!     fn update(state: &mut State, message: Message) {
//!         match message {
//!             Message::LinkClicked(url) => {
//!                 println!("The following url was clicked: {url}");
//!             }
//!         }
//!     }
//! }
//! ```
use crate::core::alignment;
use crate::core::border;
use crate::core::font::{self, Font};
use crate::core::padding;
use crate::core::theme::palette;
use crate::core::{self, Color, Element, Length, Padding, Pixels, Theme, color};
use crate::{checkbox, column, container, rich_text, row, rule, scrollable, span, text};

use std::borrow::BorrowMut;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use rustc_hash::{FxHashMap, FxHashSet};
use std::mem;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

pub use core::text::Highlight;
pub use pulldown_cmark::HeadingLevel;

/// A [`String`] representing a [URI] in a Markdown document
///
/// [URI]: https://en.wikipedia.org/wiki/Uniform_Resource_Identifier
pub type Uri = String;

mod model;
mod parser;
mod view;

pub use model::{Bullet, Column, Item, Row, Text};
pub use parser::{Content, parse};
pub use view::{Catalog, Settings, Style, Viewer, view, view_with};
