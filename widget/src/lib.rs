//! Use the built-in widgets or create your own.
#![doc(html_logo_url = "https://raw.githubusercontent.com/theaimartco/taim/main/docs/logo.svg")]
#![cfg_attr(docsrs, feature(doc_cfg))]
pub use taim_renderer as renderer;
pub use taim_renderer::core;
pub use taim_renderer::graphics;

pub use core::widget::{Id, Void};

mod action;
mod adaptive;
mod column;
mod constraint;
mod mouse_area;
mod pin;
#[cfg(feature = "signal")]
mod reactive;
mod responsive;
mod safe_area;
mod stack;
mod themer;

pub mod app_bar;

#[cfg(feature = "taim")]
#[cfg_attr(docsrs, doc(cfg(feature = "taim")))]
pub mod taim;

pub mod button;
pub mod checkbox;
pub mod combo_box;
pub mod container;
pub mod dialog;
pub mod float;
pub mod grid;
pub mod icon_button;
pub mod keyed;
pub mod list_tile;
pub mod overlay;
pub mod pane_grid;
pub mod pick_list;
pub mod progress_bar;
pub mod radio;
pub mod row;
pub mod rule;
pub mod scrollable;
pub mod sensor;
pub mod slider;
pub mod sliver;
pub mod space;
pub mod table;
pub mod text;
pub mod text_editor;
pub mod text_input;
pub mod toggler;
pub mod tooltip;
pub mod transition;
pub mod vertical_slider;
pub mod virtual_list;

pub mod accordion;
pub mod avatar;
pub mod badge;
pub mod bottom_nav_bar;
pub mod bottom_sheet;
pub mod carousel;
pub mod chip;
pub mod date;
pub mod date_picker;
pub mod drawer;
pub mod pull_to_refresh;
pub mod scaffold;
pub mod search_bar;
pub mod segmented_control;
pub mod skeleton;
pub mod snackbar;
pub mod stepper;
pub mod tabs;
pub mod time_picker;

pub mod navigator;

mod helpers;

pub use helpers::*;

#[cfg(feature = "lazy")]
mod lazy;

#[cfg(feature = "lazy")]
pub use crate::lazy::helpers::*;

#[doc(no_inline)]
pub use adaptive::Adaptive;
#[doc(no_inline)]
pub use button::Button;
#[doc(no_inline)]
pub use checkbox::Checkbox;
#[doc(no_inline)]
pub use column::Column;
#[doc(no_inline)]
pub use combo_box::ComboBox;
#[doc(no_inline)]
pub use constraint::Constraint;
#[doc(no_inline)]
pub use container::Container;
#[doc(no_inline)]
pub use float::Float;
#[doc(no_inline)]
pub use grid::Grid;
#[doc(no_inline)]
pub use mouse_area::MouseArea;
#[doc(no_inline)]
pub use pane_grid::PaneGrid;
#[doc(no_inline)]
pub use pick_list::PickList;
#[doc(no_inline)]
pub use pin::Pin;
#[doc(no_inline)]
pub use progress_bar::ProgressBar;
#[doc(no_inline)]
pub use radio::Radio;
#[cfg(feature = "signal")]
#[doc(no_inline)]
pub use reactive::{Invalidation as ReactiveInvalidation, Reactive};
#[doc(no_inline)]
pub use responsive::Responsive;
#[doc(no_inline)]
pub use row::Row;
#[doc(no_inline)]
pub use rule::Rule;
#[doc(no_inline)]
pub use safe_area::SafeArea;
#[doc(no_inline)]
pub use scrollable::Scrollable;
#[doc(no_inline)]
pub use sensor::Sensor;
#[doc(no_inline)]
pub use slider::Slider;
#[doc(no_inline)]
pub use space::Space;
#[doc(no_inline)]
pub use stack::Stack;
#[doc(no_inline)]
pub use text::Text;
#[doc(no_inline)]
pub use text_editor::TextEditor;
#[doc(no_inline)]
pub use text_input::TextInput;
#[doc(no_inline)]
pub use themer::Themer;
#[doc(no_inline)]
pub use toggler::Toggler;
#[doc(no_inline)]
pub use tooltip::Tooltip;
#[doc(no_inline)]
pub use vertical_slider::VerticalSlider;
#[doc(no_inline)]
pub use virtual_list::VirtualList;

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

pub mod icon;

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
