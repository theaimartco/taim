use crate::container;
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::widget::visit;
use crate::core::widget::{self, Tree};
use crate::core::{self, Element, Event, Layout, Padding, Point, Rectangle, Shell, Size, Vector};
use crate::pane_grid::controls::Controls;

mod behavior;
mod model;

pub use model::TitleBar;
