//! An AT-SPI2 bridge: the Linux/VYOM platform adapter.
//!
//! AT-SPI2 is how assistive technology on Linux — Orca, and VYOM's own
//! reader — reaches an application: the app exposes a tree of D-Bus
//! objects implementing `org.a11y.atspi.*`, registers with the
//! accessibility registry, and emits events as things change.
//!
//! This module is split so that the part which decides *what a user
//! hears* can be tested without a bus:
//!
//! - [`vocabulary`] — pure translation between Aura's semantic vocabulary
//!   and AT-SPI2's wire ABI. Roles, states, actions, politeness.
//! - the transport (D-Bus object server, registry handshake, event
//!   emission), which needs a session bus and an async runtime.
//!
//! Hand-rolled rather than bridged through `accesskit`, matching the rest
//! of `aura-accessibility`: the retained tree, diffing, focus engine, and
//! live-region policy are all Aura's own, and routing them through
//! another toolkit's tree model would mean maintaining a translation into
//! a second engine that duplicates this one.

pub mod adapter;
#[cfg(feature = "atspi")]
pub mod server;
pub mod store;
pub mod vocabulary;
