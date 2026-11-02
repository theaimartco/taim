//! The built-in views.
mod area;
mod button;
mod container;
mod disclosure;
mod dynamic;
mod editor;
mod forms;
mod icon;
mod layers;
mod list;
mod look;
#[cfg(any(feature = "image", feature = "svg"))]
mod media;
mod mobile;
mod navigator;
mod overlays;
mod pickers;
mod pinch;
mod range;
mod scroll;
mod space;
mod stack;
mod text;
mod text_field;
mod toggle;

pub use area::{MouseArea, mouse_area};
pub use button::{Button, Kind as ButtonKind, button};
pub use container::{Container, container};
pub use disclosure::{
    Accordion, Carousel, Stepper, Tooltip, accordion, carousel, stepper, tooltip,
};
pub use dynamic::{Each, Rebuild, each, rebuild, when};
pub use editor::{Buffer, TextEditor, text_editor};
pub use forms::{
    Dialog, Dropdown, Grid, Radio, app_bar, dialog, dropdown, grid, icon_button, list_tile, radio,
};
pub use icon::{Icon, icon};
pub use layers::{Layers, layers};
pub use list::{List, list};
#[cfg(feature = "image")]
pub use media::{Image, image};
#[cfg(feature = "svg")]
pub use media::{Svg, svg};
pub use mobile::{
    Avatar, Badge, Chip, Segments, Skeleton, avatar, badge, chip, nav_bar, scaffold, search,
    segmented, skeleton, tabs,
};
pub use navigator::{Navigator, NavigatorView, navigator};
pub use overlays::{
    Drawer, PullToRefresh, Sheet, Snackbar, drawer, pull_to_refresh, sheet, snackbar,
};
pub use pickers::{Date, DatePicker, Time, date_picker, days_in_month, is_leap_year, time_picker};
pub use pinch::{Pinch, pinch};
pub use range::{Progress, Rule, Slider, progress, rule, slider};
pub use scroll::{Scroll, scroll};
pub use space::{Space, space};
pub use stack::{Align, Stack, column, row};
pub use text::{Text, text};
pub use text_field::{TextField, text_field};
pub use toggle::{Toggle, checkbox, switch};

/// A list of children of mixed view types: `column(views![a, b, c])`.
#[macro_export]
macro_rules! views {
    ($($child:expr),* $(,)?) => {
        [$($crate::AnyView::of($child)),*]
    };
}
