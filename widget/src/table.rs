//! Data tables with retained layout metrics.

use crate::core;
use crate::core::alignment;
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::widget;
use crate::core::widget::visit;
use crate::core::{
    Alignment, Background, Element, Layout, Length, Pixels, Rectangle, Size, Widget,
};

mod control;
mod implementation;
mod style;

pub use control::{Column, Table, column, table};
pub use style::{Catalog, Style, StyleFn, default};
