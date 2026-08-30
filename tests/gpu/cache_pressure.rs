//! Public-API coverage for deterministic cache eviction and pressure
//! reporting under a tight byte budget.
//!
//! `wgpu/src/resource_budget.rs` and `graphics/src/text/cache.rs`'s own
//! unit tests already cover their internals directly. These tests
//! instead exercise both through their public API, proving two
//! end-to-end properties a real application depends on: a `Tracker`
//! correctly classifies pressure from real reported byte counts, and a
//! `text::cache::Cache` under a byte budget never silently
//! use-after-evicts an entry the current frame still needs.

use taim_core::Size;
use taim_core::cache::{Budget, Class};
use taim_core::text::fallback::{FallbackChain, Script};
use taim_core::text::paragraph::Direction;
use taim_core::text::{Alignment, Ellipsis, Shaping, Wrapping};
use taim_graphics::text::cache::{Cache, Key};
use taim_graphics::text::cosmic_text;
use taim_wgpu::resource_budget::{Pressure, Tracker};

#[test]
fn a_tracker_reports_normal_pressure_for_a_realistic_light_workload() {
    let mut tracker = Tracker::new();

    // A modest scene: a few megabytes of text/image data, well under
    // the default 256 MiB limit.
    tracker.report(Class::Essential, 4 * 1024 * 1024);
    tracker.report(Class::Retained, 2 * 1024 * 1024);

    assert_eq!(tracker.pressure(), Pressure::Normal);
}

#[test]
fn a_tracker_escalates_pressure_as_reported_usage_climbs_toward_the_limit() {
    let mut tracker = Tracker::with_limit(1_000_000);

    tracker.report(Class::Essential, 500_000);
    assert_eq!(tracker.pressure(), Pressure::Normal);

    tracker.report(Class::Essential, 750_000);
    assert_eq!(tracker.pressure(), Pressure::Elevated);

    tracker.report(Class::Essential, 950_000);
    assert_eq!(tracker.pressure(), Pressure::Critical);
}

fn key(content: &str) -> Key<'_> {
    Key {
        content,
        size: 16.0,
        line_height: 20.0,
        font: taim_core::Font::default(),
        bounds: Size::new(200.0, 200.0),
        shaping: Shaping::Basic,
        align_x: Alignment::default(),
        wrapping: Wrapping::default(),
        ellipsis: Ellipsis::default(),
        locale: None,
        direction: Direction::Auto,
        script: Script::Latin,
        fallback: FallbackChain::for_script(Script::Latin),
        axes: &[],
    }
}

#[test]
fn a_text_cache_under_a_tight_budget_never_loses_an_entry_still_needed_this_frame() {
    let mut cache = Cache::with_budget(Budget::Bytes(1));
    let mut font_system = cosmic_text::FontSystem::new();

    // Every one of these is allocated within the same "frame" (no
    // `trim` in between), so none of them may be evicted despite a
    // budget far too small to hold even one -- evicting something still
    // in use this frame would be a real rendering bug (a widget's
    // shaped text vanishing out from under it mid-frame), which matters
    // more than staying within a budget that was set unrealistically
    // low for this workload.
    let (hash_a, _) = cache.allocate(&mut font_system, key("first entry"));
    let (hash_b, _) = cache.allocate(&mut font_system, key("second entry"));
    let (hash_c, _) = cache.allocate(&mut font_system, key("third entry"));

    assert!(cache.get(&hash_a).is_some());
    assert!(cache.get(&hash_b).is_some());
    assert!(cache.get(&hash_c).is_some());
}

#[test]
fn a_text_cache_evicts_across_frames_once_the_budget_is_exceeded() {
    let mut cache = Cache::with_budget(Budget::Bytes(400));
    let mut font_system = cosmic_text::FontSystem::new();

    let (first_hash, _) = cache.allocate(&mut font_system, key("aaaaaaaaaa"));
    assert!(cache.get(&first_hash).is_some());

    // Advance to a new "frame": nothing has re-touched `first_hash`
    // since it was allocated, so it becomes evictable.
    cache.trim();

    let (second_hash, _) = cache.allocate(&mut font_system, key("bbbbbbbbbb"));

    assert!(
        cache.get(&second_hash).is_some(),
        "the newly allocated entry must always be present"
    );
    assert!(
        cache.current_bytes() <= 400,
        "current_bytes ({}) must respect the configured budget once eviction has a chance to run",
        cache.current_bytes()
    );
}
