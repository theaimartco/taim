//! The TAIM view layer.
//!
//! Applications are built from [`View`]s that own their state and react to
//! [`State`] handles instead of messages. See
//! `docs/architecture/taim-api.md` for the design.
#![cfg_attr(docsrs, feature(doc_cfg))]
mod cx;
mod layout;
mod root;
mod state;
mod tree;
mod view;

pub mod views;

/// Image handles, for [`views::image`].
#[cfg(feature = "image")]
pub mod image_handle {
    pub use taim_core::image::{FilterMethod, Handle};
}

/// Builds an SVG handle from bytes, for [`views::svg`].
#[cfg(feature = "svg")]
pub fn svg_handle(bytes: &'static [u8]) -> taim_core::svg::Handle {
    taim_core::svg::Handle::from_memory(bytes)
}

pub use cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
pub use layout::{Axis, Constraints, Sizing};
pub use root::{Response, Root, is_back};
pub use state::{IntoProp, Invalidation, Prop, Scope, State, spawn};
pub use view::{AnyView, Child, Handled, View};

pub use taim_core::{
    Background, Border, Color, Event, Padding, Pixels, Point, Rectangle, Shadow, Size, Theme,
    Vector, keyboard, mouse, semantics,
};

/// A point in time, as used for animation.
pub use taim_core::time::{Duration, Instant};

/// The renderer every view draws with, chosen when TAIM is compiled.
pub type Renderer = taim_renderer::Renderer;

/// Everything an application needs, in one import.
pub mod prelude {
    pub use crate::views;
    #[cfg(feature = "image")]
    pub use crate::views::image;
    #[cfg(feature = "svg")]
    pub use crate::views::svg;
    pub use crate::views::{
        Align, Buffer, Date, Icon, Navigator, Time, accordion, app_bar, avatar, badge, button,
        carousel, checkbox, chip, column, container, date_picker, dialog, drawer, dropdown, each,
        glass, grid, icon, icon_button, layers, list, list_tile, mouse_area, nav_bar, navigator,
        pinch, progress, pull_to_refresh, radio, rebuild, row, rule, scaffold, scroll, search,
        segmented, sheet, skeleton, slider, snackbar, space, stepper, switch, tabs, text,
        text_editor, text_field, time_picker, tooltip, when,
    };
    pub use crate::{AnyView, Scope, State, View, spawn};
}
