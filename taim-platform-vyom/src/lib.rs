//! Native VYOM platform integration.
//!
//! The backend talks to the Smithay compositor through Wayland directly.
//! Standard protocols are negotiated at runtime; VYOM-only privileges are
//! capability gated and fail closed when the compositor does not advertise
//! the corresponding authenticated global.
//!
//! Protocol/state models are portable. The native Wayland event loop and
//! its transport types are available only on Unix targets.

pub mod accessibility;
pub mod buffer;
pub mod capabilities;
pub mod clipboard;
pub mod color_management;
pub mod drag_drop;
#[cfg(unix)]
pub mod event_loop;
pub mod frame_clock;
pub mod gesture_bridge;
pub mod ime;
pub mod input;
pub mod power;
pub mod shm;
pub mod surface;
pub mod window_protocol;

pub use buffer::{BufferId, BufferPool, BufferState};
pub use capabilities::{Capabilities, Capability, Global};
pub use color_management::PanelDescription;
#[cfg(unix)]
pub use event_loop::{Backend, Error, PlatformEvent};
pub use surface::{SurfaceId, SurfaceLifecycle, SurfaceState};
