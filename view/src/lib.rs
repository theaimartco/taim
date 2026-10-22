//! The TAIM view layer.
//!
//! Applications are built from [`View`]s that own their state and react to
//! [`State`] handles instead of messages. See
//! `docs/architecture/taim-api.md` for the design.
#![cfg_attr(docsrs, feature(doc_cfg))]
mod cx;
pub mod host;
mod layout;
mod state;
mod tree;
mod view;

pub mod views;

pub use cx::{DescribeCx, EventCx, MeasureCx, PaintCx};
pub use host::Host;
pub use layout::{Axis, Constraints, Sizing};
pub use state::{IntoProp, Invalidation, Prop, Scope, State, spawn};
pub use view::{AnyView, Child, Handled, View};

pub use taim_core::{
    Background, Border, Color, Event, Padding, Pixels, Point, Rectangle, Size, Theme, Vector,
    keyboard, mouse, semantics,
};

/// A point in time, as used for animation.
pub use taim_core::time::{Duration, Instant};

/// The renderer every view draws with, chosen when TAIM is compiled.
pub type Renderer = taim_renderer::Renderer;

/// Everything an application needs, in one import.
pub mod prelude {
    pub use crate::views;
    pub use crate::views::{
        Align, Date, Icon, Time, accordion, avatar, badge, button, carousel, checkbox, chip,
        column, container, date_picker, drawer, each, icon, layers, nav_bar, progress,
        pull_to_refresh, rebuild, row, rule, scaffold, scroll, search, segmented, sheet, skeleton,
        slider, snackbar, space, stepper, switch, tabs, text, text_field, time_picker, tooltip,
        when,
    };
    pub use crate::{AnyView, Scope, State, View, spawn};
}
