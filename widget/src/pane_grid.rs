//! Pane grids let your users split regions of your application and organize layout dynamically.
//!
//!
//! This distribution of space is common in tiling window managers (like
//! [`awesome`](https://awesomewm.org/), [`i3`](https://i3wm.org/), or even
//! [`tmux`](https://github.com/tmux/tmux)).
//!
//! A [`PaneGrid`] supports:
//!
//! * Vertical and horizontal splits
//! * Tracking of the last active pane
//! * Mouse-based resizing
//! * Drag and drop to reorganize panes
//! * Hotkey support
//! * Configurable modifier keys
//! * [`State`] API to perform actions programmatically (`split`, `swap`, `resize`, etc.)
//!
//! # Example
//! ```no_run
//! # mod taim { pub mod widget { pub use taim_widget::*; } pub use taim_widget::Renderer; pub use taim_widget::core::*; }
//! # pub type Element<'a, Message> = taim_widget::core::Element<'a, Message, taim_widget::Theme, taim_widget::Renderer>;
//! #
//! use taim::widget::{pane_grid, text};
//!
//! struct State {
//!     panes: pane_grid::State<Pane>,
//! }
//!
//! enum Pane {
//!     SomePane,
//!     AnotherKindOfPane,
//! }
//!
//! enum Message {
//!     PaneDragged(pane_grid::DragEvent),
//!     PaneResized(pane_grid::ResizeEvent),
//! }
//!
//! fn view(state: &State) -> Element<'_, Message> {
//!     pane_grid(&state.panes, |pane, state, is_maximized| {
//!         pane_grid::Content::new(match state {
//!             Pane::SomePane => text("This is some pane"),
//!             Pane::AnotherKindOfPane => text("This is another kind of pane"),
//!         })
//!     })
//!     .on_drag(Message::PaneDragged)
//!     .on_resize(10, Message::PaneResized)
//!     .into()
//! }
//! ```
//! The [`pane_grid` example] showcases how to use a [`PaneGrid`] with resizing,
//! drag and drop, and hotkey support.
//!
//! [`pane_grid` example]: https://github.com/theaimartco/taim/tree/master/examples/pane_grid
mod axis;
mod configuration;
mod content;
mod controls;
mod direction;
mod draggable;
mod node;
mod pane;
mod split;
mod title_bar;

pub mod state;

pub use axis::Axis;
pub use configuration::{Configuration, LayoutError, LayoutStats};
pub use content::Content;
pub use controls::Controls;
pub use direction::Direction;
pub use draggable::Draggable;
pub use node::Node;
pub use pane::Pane;
pub use split::Split;
pub use state::State;
pub use title_bar::TitleBar;

use crate::container;
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay::{self, Group};
use crate::core::renderer;
use crate::core::touch;
use crate::core::widget;
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::visit;
use crate::core::window;
use crate::core::{
    self, Background, Border, Color, Element, Event, Layout, Length, Pixels, Point, Rectangle,
    Shell, Size, Theme, Vector, Widget,
};

const DRAG_DEADBAND_DISTANCE: f32 = 10.0;
const THICKNESS_RATIO: f32 = 25.0;

mod grid;
mod implementation;
mod interaction;
mod style;

pub use grid::PaneGrid;
pub use interaction::{DragEvent, Edge, Region, ResizeEvent, Target};
pub use style::{Catalog, Highlight, Line, Style, StyleFn, default};
