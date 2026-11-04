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
use std::time::{Duration, Instant};

use super::adapter::Events;
use super::server::Server;
use super::store::Store;

/// Backoff for discovering or reconnecting an unavailable accessibility bus.
const RETRY_MIN: Duration = Duration::from_secs(1);
const RETRY_MAX: Duration = Duration::from_secs(30);

/// A running AT-SPI bridge.
///
/// Dropping it asks the thread to stop and does **not** wait for it: a
/// shutting-down application should not block on a D-Bus round trip.
#[derive(Debug)]
pub struct Bridge {
    running: Arc<AtomicBool>,
    events: Events,
}

struct RunningGuard(Arc<AtomicBool>);
impl Drop for RunningGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl Bridge {
    /// Starts the bridge for `store`, announcing the application as `name`.
    ///
    /// Returns immediately; the connection is made on the spawned thread.
    pub fn spawn(store: Store, events: Events, name: impl Into<String>) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let name = name.into();

        let thread_running = Arc::clone(&running);

        let thread_events = events.clone();
        let exited = Arc::clone(&running);
        if let Err(error) = std::thread::Builder::new()
            .name("taim-atspi".to_owned())
            .spawn(move || {
                let _guard = RunningGuard(exited);
                run(store, thread_events, name, thread_running);
            })
        {
            running.store(false, Ordering::Release);
            log::warn!("accessibility bridge thread could not start: {error}");
        }
        Self { running, events }
    }

    /// Whether the bridge's thread is still running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.events.stop_waiting(&self.running);
    }
}

/// Bus I/O stays on this worker. Its idle wait is signalled by event publication
/// or shutdown; only an unavailable/disconnected bus needs a retry deadline.
fn run(store: Store, events: Events, name: String, running: Arc<AtomicBool>) {
    let mut server = None;
    let mut retry_delay = RETRY_MIN;
    let mut retry_at = Instant::now();
    while running.load(Ordering::Acquire) {
        let mut connected = false;
        if server.is_none() && Instant::now() >= retry_at {
            match zbus::block_on(Server::start(store.clone(), &name)) {
                Ok(Some(next)) => {
                    server = Some(next);
                    connected = true;
                    retry_delay = RETRY_MIN;
                    log::info!("accessibility bridge connected as '{name}'");
                }
                Ok(None) => {}
                Err(error) => log::debug!("accessibility bridge unavailable: {error}"),
            }
            retry_at = Instant::now() + retry_delay;
            if server.is_none() {
                retry_delay = (retry_delay * 2).min(RETRY_MAX);
            }
        }
        let drained = events.drain();
        if let Some(active) = &mut server
            && (connected || !drained.is_empty())
            && let Err(error) = zbus::block_on(active.sync())
        {
            log::warn!("accessibility sync failed; reconnecting: {error}");
            server = None;
            retry_at = Instant::now() + RETRY_MIN;
        }
        let timeout = server
            .is_none()
            .then(|| retry_at.saturating_duration_since(Instant::now()));
        events.wait_for_work(&running, timeout);
    }
}
