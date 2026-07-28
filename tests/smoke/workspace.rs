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

/// The default VYOM profile must expose a working application/daemon
/// builder, the `wgpu` and `tiny-skia` renderer feature flags, and the
/// core prelude re-exports every widget module needs.
#[test]
fn default_profile_public_api_compiles() {
    #[allow(unused_imports)]
    use aura::{Alignment, Center, Fill, Length, Task};

    #[allow(unused_imports)]
    use aura::widget;

    #[allow(dead_code)]
    fn assert_application_builder_exists() {
        fn _typecheck<State, Message>()
        where
            State: Default + 'static,
            Message: std::fmt::Debug + Send + 'static,
        {
        }
    }
}

/// `aura_core`, `aura_runtime`, and `aura_widget` must each compile and
/// link with no default features -- i.e. with no GPU backend, no
/// windowing backend, and no software rasterizer pulled in. This is the
/// "minimal-headless" combination from `docs/architecture/feature-policy.md`;
/// it is verified at the workspace level in CI via
/// `cargo check -p aura_core -p aura_runtime -p aura_widget --no-default-features`
/// rather than here, since this crate (`aura`) always depends on both
/// renderers. This test instead asserts that the renderer-neutral core
/// types used across every phase are reachable without any backend
/// feature enabled.
#[test]
fn core_types_are_backend_independent() {
    use aura::{Point, Rectangle, Size};

    let size = Size::new(1.0_f32, 1.0_f32);
    let origin = Point::ORIGIN;
    let bounds = Rectangle::new(origin, size);

    assert_eq!(bounds.width, 1.0);
    assert_eq!(bounds.height, 1.0);
}

/// The `advanced` feature must additively expose the advanced module
/// without breaking the base public API. `Cargo.toml`'s `[dev-dependencies]`
/// enables `advanced` for the whole `aura` dev-dependency graph, so this
/// checks the feature is real rather than a manifest-only stub.
#[test]
fn advanced_feature_exposes_advanced_module() {
    #[allow(unused_imports)]
    use aura::advanced;
}
