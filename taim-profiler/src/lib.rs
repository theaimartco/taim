//! Low-overhead, privacy-preserving instrumentation for Taim.
//!
//! # What this is for
//!
//! `phase.md` Phase 15's objective is to "make performance and
//! correctness observable without attaching a general-purpose debugger",
//! and its exit gate is that "every global metric can be traced to a
//! phase/cause". A frame time of 22ms is not actionable; "22ms, of which
//! 14 was `Paint`, because the text cache missed 40 times" is.
//!
//! # The two properties that shape everything here
//!
//! **It must be free when off.** [`Collector::record`] takes a closure,
//! so a disabled build never constructs the event — no clock read, no
//! tree walk. A recording site costs a relaxed atomic load and a
//! not-taken branch.
//!
//! **It must be unable to leak.** An [`Event`] cannot hold user content:
//! the only free-form string it carries is a [`Label`], which is
//! `&'static str` and can therefore only come from the program's own
//! source. Redaction is not a pass over the buffer that someone must
//! remember to extend — it is the shape of the type. See [`events`].
//!
//! # Layout
//!
//! - [`events`] — the schema, and the privacy boundary.
//! - [`collector`] — the bounded, non-blocking ring buffer.
//! - [`export`] — Chrome trace and Taim JSON.

pub mod collector;
pub mod events;
pub mod export;

pub use collector::{Collector, Dropped, Priority, Sampling};
pub use events::{
    Cause, Event, Label, Nanos, Phase, Redacted, Resource, SCHEMA_VERSION, Sensitivity,
};
