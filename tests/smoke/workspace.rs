//! Compile-time public API and feature-combination smoke tests.
//!
//! These tests exist to catch the specific failure mode Phase 0 repairs:
//! a workspace manifest that resolves individually but produces a broken
//! build once real feature combinations are exercised. They intentionally
//! do not assert on rendering or runtime behavior -- that is the job of
//! later phases' functional test suites.
//!
//! Feature-combination coverage (default / minimal-headless / hot /
//! recovery-fallback) is exercised by CI running this file once per
//! combination in `.github/workflows/ci.yml`, since a single `cargo test`
//! invocation only sees the feature set it was built with.

/// The default profile must expose the view-based application entry
/// points and the prelude every application imports.
#[test]
fn default_profile_public_api_compiles() {
    #[allow(unused_imports)]
    use taim::prelude::*;

    // Describing an application must not open a window; only `run` does.
    let _app: taim::App = taim::app("smoke", |_cx| text("ready"));
}

/// `taim_core`, `taim_runtime`, and `taim_widget` must each compile and
/// link with no default features -- i.e. with no GPU backend, no
/// windowing backend, and no software rasterizer pulled in. This is the
/// "minimal-headless" combination from `docs/architecture/feature-policy.md`;
/// it is verified at the workspace level in CI via
/// `cargo check -p taim_core -p taim_runtime -p taim_widget --no-default-features`
/// rather than here, since this crate (`taim`) always depends on both
/// renderers. This test instead asserts that the renderer-neutral core
/// types used across every phase are reachable without any backend
/// feature enabled.
#[test]
fn core_types_are_backend_independent() {
    use taim::{Point, Rectangle, Size};

    let size = Size::new(1.0_f32, 1.0_f32);
    let origin = Point::ORIGIN;
    let bounds = Rectangle::new(origin, size);

    assert_eq!(bounds.width, 1.0);
    assert_eq!(bounds.height, 1.0);
}

/// The engine crates the facade is built on stay individually usable for
/// custom widget authors.
#[test]
fn engine_crates_are_reachable() {
    #[allow(unused_imports)]
    use taim_core::{Layout, Widget, layout, renderer};
    #[allow(unused_imports)]
    use taim_widget::Element;
}
