//! Stable public namespaces assembled from TAIM's focused crates.
pub use taim_futures::{Subscription, futures, stream};
pub use taim_signal::{Memo, Signal};

#[cfg(feature = "highlighter")]
pub use taim_highlighter as highlighter;

#[cfg(feature = "wgpu-bare")]
pub use taim_renderer::wgpu::wgpu;

pub use crate::application::{Application, ReactiveApplication};
pub use crate::core::alignment;
pub use crate::core::animation;
pub use crate::core::border;
pub use crate::core::color;
pub use crate::core::gradient;
pub use crate::core::padding;
pub use crate::core::scene::{NodeId, SceneArena, SceneNode};
pub use crate::core::theme;
pub use crate::core::{
    Alignment, Animation, Background, Border, Color, ContentFit, Degrees, Function, Gradient,
    Length, Never, Padding, Pixels, Point, Radians, Rectangle, Rotation, Settings, Shadow, Size,
    Theme, Transformation, Vector, never,
};
pub use crate::core::{SemanticsDriver, layout_solver, scene};
pub use crate::daemon::Daemon;
pub use crate::error::Error;
pub use crate::program::Preset;
pub use crate::program::message;
pub use crate::program::{
    App, AppConfig, AppContext, AppRunner, FrameRequest, Program, ProgramAdapter, RuntimeContext,
    WindowContext,
};
pub use crate::runtime::{ReactiveUi, exit};
pub use crate::window::Window;
pub use taim_widget::renderer::Renderer;

#[doc(inline)]
pub use crate::application::{application, reactive_app};
#[doc(inline)]
pub use crate::daemon::daemon;

pub use Alignment::Center;
pub use Length::{Fill, FillPortion, Fit, Shrink};
pub use alignment::Horizontal::{Left, Right};
pub use alignment::Vertical::{Bottom, Top};

/// The VYOM backend for TAIM applications.
#[cfg(feature = "vyom")]
pub mod platform_vyom {
    pub use taim_platform_vyom::*;
}

/// Fine-grained reactive state primitives.
pub mod signal {
    pub use taim_signal::*;
}

#[cfg(feature = "signal")]
pub mod component {
    //! Retained reactive components.
    pub use taim_widget::component::*;
}

/// Debug instrumentation helpers.
pub mod debug {
    pub use taim_debug::{Span, time, time_with};
}

/// Runtime task types and constructors.
pub mod task {
    pub use crate::runtime::task::{Handle, Task};

    #[cfg(feature = "sipper")]
    pub use crate::runtime::task::{Never, Sipper, Straw, sipper, stream};
}

pub use task::Task;

/// Clipboard content types and asynchronous operations.
pub mod clipboard {
    pub use crate::core::clipboard::{Content, Error, Kind};
    pub use crate::runtime::clipboard::{read, read_files, read_html, read_text, write};

    #[cfg(feature = "image")]
    pub use crate::core::clipboard::Image;
    #[cfg(feature = "image")]
    pub use crate::runtime::clipboard::read_image;
}

/// Executor selection.
pub mod executor {
    pub use taim_futures::Executor;
    pub use taim_futures::backend::default::Executor as Default;
}

pub use executor::Executor;

/// Font types and loading operations.
pub mod font {
    pub use crate::core::font::*;
    pub use crate::runtime::font::*;
}

pub use font::Font;

/// Runtime and UI events.
pub mod event {
    pub use crate::core::event::{Event, Status};
    pub use taim_futures::event::{listen, listen_raw, listen_url, listen_with};
}

pub use event::Event;

/// Keyboard input and subscriptions.
pub mod keyboard {
    pub use crate::core::keyboard::key;
    pub use crate::core::keyboard::{Event, Key, Location, Modifiers};
    pub use taim_futures::keyboard::listen;
}

/// Mouse input types.
pub mod mouse {
    pub use crate::core::mouse::{Button, Cursor, Event, Interaction, ScrollDelta};
}

/// System information and appearance subscriptions.
pub mod system {
    pub use crate::runtime::system::{theme, theme_changes};

    #[cfg(feature = "sysinfo")]
    pub use crate::runtime::system::{Information, information};
}

/// Interactive content drawn above the normal widget tree.
pub mod overlay {
    /// A generic overlay using TAIM's default theme and renderer.
    pub type Element<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer> =
        crate::core::overlay::Element<'a, Message, Theme, Renderer>;

    pub use taim_widget::overlay::*;
}

/// Touch input types.
pub mod touch {
    pub use crate::core::touch::{Event, Finger};
}

#[allow(hidden_glob_reexports)]
pub mod widget {
    //! Built-in widgets and widget runtime operations.
    pub use taim_runtime::widget::*;
    pub use taim_widget::*;

    #[cfg(feature = "image-without-codecs")]
    /// Raster image widgets without bundled format codecs.
    pub mod image {
        pub use taim_runtime::image::{Allocation, Error, allocate};
        pub use taim_widget::image::*;
    }

    mod core {}
    mod graphics {}
    mod renderer {}
}

/// Renderer backend selection and preferences.
pub mod backend {
    pub use taim_core::backend::*;
    pub use taim_runtime::backend::*;
}

pub use backend::{Backend, PowerPreference};

/// A widget element using TAIM's default theme and renderer.
pub type Element<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer> =
    crate::core::Element<'a, Message, Theme, Renderer>;

/// The result returned by a TAIM runner.
pub type Result = std::result::Result<(), Error>;
