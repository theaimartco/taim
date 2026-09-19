//! Graphics backend selection, configuration, and initialization errors.

mod error;
mod selection;
mod settings;

pub use error::{Error, Reason};
pub use selection::{Api, Backend, PowerPreference};
pub use settings::Settings;
