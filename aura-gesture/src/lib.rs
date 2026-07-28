//! A unified pointer gesture arena for `aura`.
//!
//! Real touch/pointer interaction is rarely "one widget wants one
//! event": a horizontal pan inside a vertically-scrolling list, a
//! long-press that must not fire if the finger was actually starting a
//! drag, a two-finger pinch that a single-finger pan recognizer must not
//! also claim -- all of these require several candidate interpretations
//! of the *same* pointer stream to coexist until enough evidence rules
//! all but one of them out. That coexistence-then-resolution structure
//! is what [`arena::Arena`] provides: a bounded set of
//! [`recognizer::Recognizer`]s registered against one contact "enter the
//! arena together, and are resolved down to at most one winner,
//! following the correctness rules this crate is built around:
//!
//! - Every pointer sequence ends in up or cancel for all participants.
//! - Losing recognizers receive cancellation exactly once.
//! - Captured pointers keep routing outside their original bounds.
//! - High-frequency moves may be coalesced; down/up/cancel and semantic
//!   boundary events may not be dropped (see
//!   `aura_core::event::Coalescing`).
//!
//! # Module map
//!
//! - [`recognizer`]: the trait every gesture implements, and the shared
//!   [`recognizer::Disposition`]/[`recognizer::Verdict`] vocabulary the
//!   arena resolves competitions with.
//! - [`arena`]: the competition/resolution/capture engine itself.
//! - [`velocity`]: a small, timestamped, bounded velocity estimator used
//!   by [`swipe`] and any recognizer that cares about how fast a
//!   pointer was moving, not just where it ended up.
//! - [`tap`], [`long_press`], [`pan`], [`swipe`], [`edge_swipe`],
//!   [`pinch`], [`rotate`]: the concrete recognizers.
//! - [`nested_scroll`]: the child/parent handoff coordinator for nested
//!   scrollable regions (pre/post consumption and overscroll), built on
//!   top of the arena rather than being a recognizer itself.
pub mod arena;
pub mod edge_swipe;
pub mod long_press;
pub mod nested_scroll;
pub mod pan;
pub mod pinch;
pub mod recognizer;
pub mod rotate;
pub mod swipe;
pub mod tap;
pub mod velocity;

pub use arena::Arena;
pub use recognizer::{Disposition, Recognizer, Verdict};
