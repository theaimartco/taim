//! Use the built-in widgets or create your own.
#![doc(html_logo_url = "https://raw.githubusercontent.com/theaimartco/taim/main/docs/logo-256.png")]
#![cfg_attr(docsrs, feature(doc_cfg))]
pub use taim_renderer as renderer;
pub use taim_renderer::core;
pub use taim_renderer::graphics;

pub use core::widget::{Id, Void};

macro_rules! private_modules {
    ($($name:ident),+ $(,)?) => { $(mod $name;)+ };
}

macro_rules! public_modules {
    ($($name:ident),+ $(,)?) => { $(pub mod $name;)+ };
}

macro_rules! export_widgets {
    ($($module:ident::$widget:ident),+ $(,)?) => {
        $(
            #[doc(no_inline)]
            pub use $module::$widget;
        )+
    };
}

/// Broad role of a built-in TAIM widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WidgetFamily {
    /// Spatial arrangement or responsive layout.
    Layout,
    /// Direct user input.
    Input,
    /// Movement between application destinations.
    Navigation,
    /// Status, progress, or transient feedback.
    Feedback,
    /// Structured content presentation.
    Data,
    /// Raster, vector, canvas, or GPU-authored content.
    Media,
}

/// Discoverable metadata for one built-in widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WidgetDescriptor {
    /// Stable snake-case widget name.
    pub name: &'static str,
    /// Primary role of the widget.
    pub family: WidgetFamily,
    /// Cargo feature required to compile the widget, if any.
    pub feature: Option<&'static str>,
}

impl WidgetDescriptor {
    /// Returns whether the descriptor is available in this build.
    pub fn is_available(self) -> bool {
        match self.feature {
            None => true,
            Some("canvas") => cfg!(feature = "canvas"),
            Some("image") => cfg!(feature = "image"),
            Some("lazy") => cfg!(feature = "lazy"),
            Some("markdown") => cfg!(feature = "markdown"),
            Some("qr_code") => cfg!(feature = "qr_code"),
            Some("signal") => cfg!(feature = "signal"),
            Some("svg") => cfg!(feature = "svg"),
            Some("wgpu") => cfg!(feature = "wgpu"),
            Some(_) => false,
        }
    }
}

const fn widget(
    name: &'static str,
    family: WidgetFamily,
    feature: Option<&'static str>,
) -> WidgetDescriptor {
    WidgetDescriptor {
        name,
        family,
        feature,
    }
}

/// Alphabetical catalog used by tooling, documentation, and widget pickers.
pub const WIDGET_CATALOG: &[WidgetDescriptor] = &[
    widget("accordion", WidgetFamily::Data, None),
    widget("adaptive", WidgetFamily::Layout, None),
    widget("app_bar", WidgetFamily::Navigation, None),
    widget("avatar", WidgetFamily::Data, None),
    widget("badge", WidgetFamily::Feedback, None),
    widget("bottom_nav_bar", WidgetFamily::Navigation, None),
    widget("bottom_sheet", WidgetFamily::Navigation, None),
    widget("button", WidgetFamily::Input, None),
    widget("canvas", WidgetFamily::Media, Some("canvas")),
    widget("carousel", WidgetFamily::Navigation, None),
    widget("checkbox", WidgetFamily::Input, None),
    widget("chip", WidgetFamily::Input, None),
    widget("column", WidgetFamily::Layout, None),
    widget("combo_box", WidgetFamily::Input, None),
    widget("constraint", WidgetFamily::Layout, None),
    widget("container", WidgetFamily::Layout, None),
    widget("date", WidgetFamily::Data, None),
    widget("date_picker", WidgetFamily::Input, None),
    widget("dialog", WidgetFamily::Feedback, None),
    widget("drawer", WidgetFamily::Navigation, None),
    widget("float", WidgetFamily::Layout, None),
    widget("grid", WidgetFamily::Layout, None),
    widget("icon", WidgetFamily::Media, None),
    widget("icon_button", WidgetFamily::Input, None),
    widget("image", WidgetFamily::Media, Some("image")),
    widget("keyed", WidgetFamily::Layout, None),
    widget("list_tile", WidgetFamily::Data, None),
    widget("markdown", WidgetFamily::Data, Some("markdown")),
    widget("mouse_area", WidgetFamily::Input, None),
    widget("navigator", WidgetFamily::Navigation, None),
    widget("overlay", WidgetFamily::Layout, None),
    widget("pane_grid", WidgetFamily::Layout, None),
    widget("pick_list", WidgetFamily::Input, None),
    widget("pin", WidgetFamily::Layout, None),
    widget("progress_bar", WidgetFamily::Feedback, None),
    widget("pull_to_refresh", WidgetFamily::Feedback, None),
    widget("qr_code", WidgetFamily::Media, Some("qr_code")),
    widget("radio", WidgetFamily::Input, None),
    widget("responsive", WidgetFamily::Layout, None),
    widget("row", WidgetFamily::Layout, None),
    widget("rule", WidgetFamily::Data, None),
    widget("safe_area", WidgetFamily::Layout, None),
    widget("scaffold", WidgetFamily::Layout, None),
    widget("scrollable", WidgetFamily::Layout, None),
    widget("search_bar", WidgetFamily::Input, None),
    widget("segmented_control", WidgetFamily::Input, None),
    widget("sensor", WidgetFamily::Input, None),
    widget("shader", WidgetFamily::Media, Some("wgpu")),
    widget("skeleton", WidgetFamily::Feedback, None),
    widget("slider", WidgetFamily::Input, None),
    widget("sliver", WidgetFamily::Layout, None),
    widget("snackbar", WidgetFamily::Feedback, None),
    widget("space", WidgetFamily::Layout, None),
    widget("stack", WidgetFamily::Layout, None),
    widget("stepper", WidgetFamily::Input, None),
    widget("svg", WidgetFamily::Media, Some("svg")),
    widget("table", WidgetFamily::Data, None),
    widget("tabs", WidgetFamily::Navigation, None),
    widget("text", WidgetFamily::Data, None),
    widget("text_editor", WidgetFamily::Input, None),
    widget("text_input", WidgetFamily::Input, None),
    widget("time_picker", WidgetFamily::Input, None),
    widget("toggler", WidgetFamily::Input, None),
    widget("tooltip", WidgetFamily::Feedback, None),
    widget("transition", WidgetFamily::Feedback, None),
    widget("vertical_slider", WidgetFamily::Input, None),
    widget("virtual_list", WidgetFamily::Data, None),
];

/// Looks up built-in widget metadata without allocating.
pub fn describe(name: &str) -> Option<WidgetDescriptor> {
    WIDGET_CATALOG
        .binary_search_by(|candidate| candidate.name.cmp(name))
        .ok()
        .map(|index| WIDGET_CATALOG[index])
}

private_modules!(
    action, adaptive, column, constraint, mouse_area, pin, responsive, safe_area, stack, themer,
);

#[cfg(feature = "signal")]
pub mod component;
#[cfg(feature = "signal")]
mod reactive;

#[cfg(feature = "taim")]
#[cfg_attr(docsrs, doc(cfg(feature = "taim")))]
pub mod taim;

public_modules!(
    accordion,
    app_bar,
    avatar,
    badge,
    bottom_nav_bar,
    bottom_sheet,
    button,
    carousel,
    checkbox,
    chip,
    combo_box,
    container,
    date,
    date_picker,
    dialog,
    drawer,
    float,
    grid,
    icon,
    icon_button,
    keyed,
    list_tile,
    navigator,
    overlay,
    pane_grid,
    pick_list,
    progress_bar,
    pull_to_refresh,
    radio,
    row,
    rule,
    scaffold,
    scrollable,
    search_bar,
    segmented_control,
    sensor,
    skeleton,
    slider,
    sliver,
    snackbar,
    space,
    stepper,
    table,
    tabs,
    text,
    text_editor,
    text_input,
    time_picker,
    toggler,
    tooltip,
    transition,
    vertical_slider,
    virtual_list,
);

mod helpers;
mod linear;

pub use helpers::*;

#[cfg(feature = "lazy")]
mod lazy;

#[cfg(feature = "lazy")]
pub use crate::lazy::helpers::*;

export_widgets!(
    adaptive::Adaptive,
    button::Button,
    checkbox::Checkbox,
    column::Column,
    combo_box::ComboBox,
    constraint::Constraint,
    container::Container,
    float::Float,
    grid::Grid,
    mouse_area::MouseArea,
    pane_grid::PaneGrid,
    pick_list::PickList,
    pin::Pin,
    progress_bar::ProgressBar,
    radio::Radio,
    responsive::Responsive,
    row::Row,
    rule::Rule,
    safe_area::SafeArea,
    scrollable::Scrollable,
    sensor::Sensor,
    slider::Slider,
    space::Space,
    stack::Stack,
    text::Text,
    text_editor::TextEditor,
    text_input::TextInput,
    themer::Themer,
    toggler::Toggler,
    tooltip::Tooltip,
    vertical_slider::VerticalSlider,
    virtual_list::VirtualList,
);

#[cfg(feature = "signal")]
pub use component::{ReactiveButton, ReactiveMount, ReactiveText, ReactiveVirtualList};
#[cfg(feature = "signal")]
#[doc(no_inline)]
pub use reactive::{Invalidation as ReactiveInvalidation, Reactive};

#[cfg(feature = "wgpu")]
pub mod shader;

#[cfg(feature = "wgpu")]
#[doc(no_inline)]
pub use shader::Shader;

#[cfg(feature = "svg")]
pub mod svg;

#[cfg(feature = "svg")]
#[doc(no_inline)]
pub use svg::Svg;

#[cfg(feature = "image")]
pub mod image;

#[cfg(feature = "image")]
#[doc(no_inline)]
pub use image::Image;

#[cfg(feature = "canvas")]
pub mod canvas;

#[cfg(feature = "canvas")]
#[doc(no_inline)]
pub use canvas::Canvas;

#[cfg(feature = "qr_code")]
pub mod qr_code;

#[cfg(feature = "qr_code")]
#[doc(no_inline)]
pub use qr_code::QRCode;

#[cfg(feature = "markdown")]
pub mod markdown;

pub use crate::core::theme::{self, Theme};
pub use action::Action;
pub use renderer::Renderer;

/// An [`Element`](core::Element) drawn with the built-in [`Theme`] and
/// [`Renderer`] unless told otherwise.
pub type Element<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer> =
    core::Element<'a, Message, Theme, Renderer>;
