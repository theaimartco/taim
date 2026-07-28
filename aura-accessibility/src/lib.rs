//! Aura's retained, incremental semantic engine.
//!
//! `phase.md` Phase 13's objective is to "make accessibility a retained,
//! incremental core feature rather than a late adapter." Concretely, that
//! means the accessibility tree is a **first-class, long-lived
//! structure** that is *diffed* as the interface changes, not rebuilt and
//! re-announced from scratch:
//!
//! - [`tree::Tree`] indexes a semantic forest by stable
//!   [`aura_core::widget::Id`], remembering each node's parent and
//!   traversal order.
//! - [`tree::Tree::diff`] reports the minimal [`tree::Change`] set between
//!   two trees. Its correctness bar is exact: applying a diff to the old
//!   tree must produce something indistinguishable from a full rebuild
//!   (the exit gate, proven in `tests/accessibility/tree.rs`).
//! - [`focus`] owns traversal order, focus scopes/traps, and restoration,
//!   deterministically in both text directions and through virtualized
//!   content.
//! - [`live_region`] queues announcements with the privacy rules a live
//!   region needs, so a secure field or a locked-screen notification can
//!   never be spoken aloud.
//! - [`actions`] routes an incoming platform action back to the stable
//!   widget identity that claimed it.
//! - [`adapter`] is the seam a real platform bridge plugs into.
//!
//! # Where this sits
//!
//! `aura_runtime::semantics::build` produces the semantic forest from a
//! widget tree, already skipping clean subtrees via their dirty flags.
//! This crate takes that output and gives it identity, order, diffing,
//! focus, and delivery. It is renderer- and platform-neutral: it depends
//! only on `aura_core`, so a platform adapter can live anywhere.

pub mod actions;
pub mod adapter;
pub mod atspi;
pub mod focus;
pub mod live_region;
pub mod node;
pub mod tree;

pub use actions::{Dispatch, Rejection, Router};
pub use adapter::{Adapter, Capabilities, Session};
pub use focus::{Direction, Focus, Scope};
pub use live_region::{Announcement, LiveRegions, Refusal, Urgency};
pub use node::{Entry, Order};
pub use tree::{Change, Tree};
