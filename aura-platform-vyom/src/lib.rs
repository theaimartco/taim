//! Native VYOM platform integration.
//!
//! The backend talks to the Smithay compositor through Wayland directly.
//! Standard protocols are negotiated at runtime; VYOM-only privileges are
//! capability gated and fail closed when the compositor does not advertise
//! the corresponding authenticated global.

pub mod accessibility;
pub mod capabilities;
pub mod clipboard;
pub mod drag_drop;
pub mod event_loop;
pub mod frame_clock;
pub mod gesture_bridge;
pub mod ime;
pub mod input;
pub mod power;
pub mod surface;
pub mod window_protocol;

pub use capabilities::{Capabilities, Capability, Global};
pub use event_loop::{Backend, Error, PlatformEvent};
pub use surface::{SurfaceId, SurfaceLifecycle, SurfaceState};
