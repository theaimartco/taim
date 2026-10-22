//! The built-in views.
mod button;
mod container;
mod disclosure;
mod dynamic;
mod forms;
mod icon;
mod layers;
mod look;
mod mobile;
mod overlays;
mod pickers;
mod range;
mod scroll;
mod space;
mod stack;
mod text;
mod text_field;
mod toggle;

pub use button::{Button, button};
pub use container::{Container, container};
pub use disclosure::{
    Accordion, Carousel, Stepper, Tooltip, accordion, carousel, stepper, tooltip,
};
pub use dynamic::{Each, Rebuild, each, rebuild, when};
pub use forms::{
    Dialog, Dropdown, Grid, Radio, app_bar, dialog, dropdown, grid, icon_button, list_tile, radio,
};
pub use icon::{Icon, icon};
pub use layers::{Layers, layers};
pub use mobile::{
    Avatar, Badge, Chip, Segments, Skeleton, avatar, badge, chip, nav_bar, scaffold, search,
    segmented, skeleton, tabs,
};
pub use overlays::{
    Drawer, PullToRefresh, Sheet, Snackbar, drawer, pull_to_refresh, sheet, snackbar,
};
pub use pickers::{Date, DatePicker, Time, date_picker, days_in_month, is_leap_year, time_picker};
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
