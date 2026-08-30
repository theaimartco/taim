//! Runs the AT-SPI server, so the semantic tree reaches assistive technology.
//!
//! # Why this exists
//!
//! Everything either side of it was already built. `Session` diffs the
//! semantic tree every frame and pushes the result into a [`Store`] and an
//! [`Events`] queue; [`Server`](super::server::Server) serves that store over
//! D-Bus as `org.a11y.atspi.*` objects. Nothing joined them, so the tree was
//! diffed, translated, and pushed into an in-memory queue that no one drained
//! — for as long as the crate has existed. Orca could not read an Taim
//! application, and the framework's most distinctive claim, "accessibility by
//! construction", was a claim about a `VecDeque`.
//!
//! # Why a thread rather than the UI loop
//!
//! D-Bus is a socket. A screen reader that stops reading, a registry that is
//! slow to answer, a bus that goes away mid-call — each of those blocks, and
//! none of them may block a frame. The bridge therefore owns a thread and an
//! executor of its own, and communicates with the UI thread only through the
//! two lock-free-enough structures it was given, both of which are `Arc`-
//! backed and cheap to clone.
//!
//! The UI thread never waits on this thread, and this thread never touches
//! the widget tree.
//!
//! # Failing to start is not an error
//!
//! With no accessibility bus running — the ordinary case on a desktop with no
//! assistive technology — [`Server::start`] returns `Ok(None)` and this
//! bridge parks. An application must not fail to launch because nobody is
//! listening, and must not pay for a bus that is not there.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::adapter::Events;
use super::server::Server;
use super::store::Store;

/// How long the bridge waits between drains when the queue is empty.
///
/// A poll rather than a condvar: the queue is written from the UI thread on
/// every frame that changes anything, and 20 ms is far below the threshold at
/// which a screen reader user notices latency, while costing one wakeup per
/// 50th of a second on an idle application.
const IDLE_POLL: Duration = Duration::from_millis(20);

/// A running AT-SPI bridge.
///
/// Dropping it asks the thread to stop and does **not** wait for it: a
/// shutting-down application should not block on a D-Bus round trip.
#[derive(Debug)]
pub struct Bridge {
    running: Arc<AtomicBool>,
}

impl Bridge {
    /// Starts the bridge for `store`, announcing the application as `name`.
    ///
    /// Returns immediately; the connection is made on the spawned thread.
    pub fn spawn(store: Store, events: Events, name: impl Into<String>) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let name = name.into();

        let thread_running = Arc::clone(&running);

        // Not a scoped thread and not joined on drop: see the type's docs.
        let _ = std::thread::Builder::new()
            .name("taim-atspi".to_owned())
            .spawn(move || {
                run(store, events, name, thread_running);
            });

        Self { running }
    }

    /// Whether the bridge's thread is still running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
    }
}

/// The bridge's loop.
///
/// Synchronous, with each D-Bus call driven by `zbus::block_on`. The thread
/// belongs to the bridge and has nothing else to do, so an async timer would
/// buy nothing over an ordinary sleep, and this way the bridge does not pin
/// the crate to a particular async runtime.
fn run(store: Store, events: Events, name: String, running: Arc<AtomicBool>) {
    let server = match zbus::block_on(Server::start(store, &name)) {
        Ok(Some(server)) => Some(server),
        // No bus: park rather than spin. The ordinary case with no assistive
        // technology running.
        Ok(None) => None,
        Err(error) => {
            log::warn!("accessibility bridge could not start: {error}");
            None
        }
    };

    let Some(mut server) = server else {
        // Still drain, so the UI thread's bounded queue cannot fill and begin
        // dropping the oldest events.
        while running.load(Ordering::Acquire) {
            let _ = events.drain();
            std::thread::sleep(IDLE_POLL);
        }

        return;
    };

    log::info!("accessibility bridge connected as '{name}'");

    while running.load(Ordering::Acquire) {
        let drained = events.drain();

        if !drained.is_empty()
            && let Err(error) = zbus::block_on(server.sync())
        {
            // A failed sync is not fatal: objects already served keep reading
            // through to the store, and the next drain retries.
            log::warn!("accessibility sync failed: {error}");
        }

        std::thread::sleep(IDLE_POLL);
    }
}
