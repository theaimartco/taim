//! The bridge that carries the semantic tree to the accessibility bus.
//!
//! `atspi_bus.rs` proves the *server* works: serve a tree, connect a second
//! client the way Orca does, ask it questions. This proves the piece that was
//! missing between that server and a running application -- the thing that
//! drains the queue the UI thread writes to every frame.
//!
//! Without it, `Session::publish` diffed the tree, translated it, and pushed
//! the result into a bounded `VecDeque` that nothing read, for as long as the
//! crate existed (`Capstone.md` F-15).

#![cfg(feature = "atspi")]

use std::time::{Duration, Instant};

use aura_accessibility::atspi::adapter::{Event, Events};
use aura_accessibility::atspi::bridge::Bridge;
use aura_accessibility::atspi::store::Store;
use aura_core::widget::Id;

/// Waits for `condition`, up to `limit`.
///
/// The bridge runs on its own thread and polls; a bare sleep would either be
/// flaky or slow, and this is neither.
fn within(limit: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + limit;

    while Instant::now() < deadline {
        if condition() {
            return true;
        }

        std::thread::sleep(Duration::from_millis(5));
    }

    condition()
}

#[test]
fn the_bridge_drains_what_the_ui_thread_publishes() {
    let events = Events::default();

    // Queued before the bridge starts, which is the real ordering: a window
    // publishes its first frame's semantics as it opens.
    events.push(Event::Focused {
        id: Some(Id::unique()),
    });
    events.push(Event::Added { id: Id::unique() });

    assert_eq!(events.len(), 2, "the queue must start with work in it");

    let bridge = Bridge::spawn(Store::new(), events.clone(), "Aura bridge test");

    assert!(
        within(Duration::from_secs(5), || events.is_empty()),
        "the bridge left {} event(s) in the queue; nothing is draining it, \
         which is the exact failure this bridge exists to fix",
        events.len()
    );

    assert!(bridge.is_running(), "the bridge thread died");
}

#[test]
fn the_bridge_keeps_draining_after_it_has_started() {
    let events = Events::default();
    let bridge = Bridge::spawn(Store::new(), events.clone(), "Aura bridge test");

    assert!(within(Duration::from_secs(5), || bridge.is_running()));

    // A second frame's worth, published after the connection is up.
    for _ in 0..8 {
        events.push(Event::Updated { id: Id::unique() });
    }

    assert!(
        within(Duration::from_secs(5), || events.is_empty()),
        "the bridge stopped draining after its first pass"
    );
}

#[test]
fn dropping_the_bridge_stops_its_thread() {
    // A window that closes must not leave a thread polling a bus forever.
    let events = Events::default();
    let bridge = Bridge::spawn(Store::new(), events.clone(), "Aura bridge test");

    assert!(within(Duration::from_secs(5), || bridge.is_running()));

    drop(bridge);

    // After the drop the queue is nobody's responsibility, so the only
    // observable is that pushing no longer gets drained.
    std::thread::sleep(Duration::from_millis(120));
    events.push(Event::Added { id: Id::unique() });
    std::thread::sleep(Duration::from_millis(120));

    assert_eq!(
        events.len(),
        1,
        "a dropped bridge is still draining; its thread outlived its window"
    );
}
