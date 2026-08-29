//! Golden tests for visual correctness of the damage tracking system.
//!
//! # These describe the production colour pipeline
//!
//! A committed golden is one image, so it can only describe one compositing
//! space. These are captured in **linear light**, which is what the default
//! build composites in and what ships.
//!
//! Under the `web-colors` compatibility feature the renderer blends
//! gamma-encoded components *by design*, so it legitimately produces different
//! pixels — a 24%-coverage edge differs by up to 73/255, which is the analytic
//! maximum between the two spaces. Comparing linear goldens against a
//! gamma-space render would fail on every antialiased edge in the suite.
//!
//! The image-comparing modules are therefore compiled only for the
//! configuration they describe. The alternatives were both worse: a second set
//! of goldens doubles the maintenance of a legacy mode, and a tolerance wide
//! enough to span 73/255 would stop detecting real regressions.
//!
//! What still runs under `web-colors`: `tests/color/contract.rs`, which asserts
//! that the legacy mode really is gamma-space, and `tests/gpu/color_output.rs`,
//! which asserts what each mode puts on the presentation target. Neither
//! compares against a stored picture, so neither has this problem.
#[path = "golden/harness.rs"]
pub mod harness;

/// Compares rendered output against committed images, which are captured in
/// the production (linear) colour space. See this file's documentation.
#[cfg(not(feature = "web-colors"))]
#[path = "golden/damage_motion.rs"]
pub mod damage_motion;

#[path = "golden/gpu_primitives.rs"]
pub mod gpu_primitives;

/// Compares rendered output against committed images, which are captured in
/// the production (linear) colour space. See this file's documentation.
#[cfg(not(feature = "web-colors"))]
#[path = "golden/transitions.rs"]
pub mod transitions;

/// Compares rendered output against committed images, which are captured in
/// the production (linear) colour space. See this file's documentation.
#[cfg(not(feature = "web-colors"))]
#[path = "golden/multilingual_text.rs"]
pub mod multilingual_text;

/// Compares rendered output against committed images, which are captured in
/// the production (linear) colour space. See this file's documentation.
#[cfg(not(feature = "web-colors"))]
#[path = "golden/design_system.rs"]
pub mod design_system;

/// Compares rendered output against committed images, which are captured in
/// the production (linear) colour space. See this file's documentation.
#[cfg(not(feature = "web-colors"))]
#[path = "golden/effects.rs"]
pub mod effects;

/// Compares rendered output against committed images, which are captured in
/// the production (linear) colour space. See this file's documentation.
#[cfg(not(feature = "web-colors"))]
#[path = "golden/components.rs"]
pub mod components;

/// Compares rendered output against committed images, which are captured in
/// the production (linear) colour space. See this file's documentation.
#[cfg(not(feature = "web-colors"))]
#[path = "golden/widget_gallery.rs"]
pub mod widget_gallery;

/// Compares rendered output against committed images, which are captured in
/// the production (linear) colour space. See this file's documentation.
#[cfg(not(feature = "web-colors"))]
#[path = "golden/damage_pixels.rs"]
pub mod damage_pixels;

/// Compares rendered output against committed images, which are captured in
/// the production (linear) colour space. See this file's documentation.
#[cfg(not(feature = "web-colors"))]
#[path = "golden/effects_gpu.rs"]
pub mod effects_gpu;
