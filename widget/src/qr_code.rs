//! Cached, styled QR-code rendering.

use crate::Renderer;
use crate::canvas;
use crate::core::layout;
use crate::core::mouse;
use crate::core::renderer::{self, Renderer as _};
use crate::core::widget::tree::Tree;
use crate::core::{
    Color, Element, Layout, Length, Pixels, Point, Rectangle, Size, Theme, Vector, Widget,
};

use std::cell::RefCell;
use thiserror::Error as ThisError;

mod control;
mod implementation;
mod model;
mod style;

pub use control::QRCode;
pub use model::{Data, Error, ErrorCorrection, Version};
pub use style::{Catalog, Style, StyleFn, default};

pub(super) const DEFAULT_CELL_SIZE: f32 = 4.0;
pub(super) const QUIET_ZONE: usize = 2;
