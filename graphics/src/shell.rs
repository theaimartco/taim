//! TAIM renderer-to-window intent delivery.
//!
//! Scenes can issue several paint/layout/tick notifications in one pass.
//! FrameBatch consolidates repeated requests and commits them at most once
//! per transaction. Immediate methods preserve the legacy shell contract.

use std::sync::Arc;

/// Shared window notifier for one renderer or its clones.
#[derive(Clone)]
pub struct Shell(Arc<dyn Notifier>);

impl Shell {
    /// Construct a platform-linked renderer notification shell.
    pub fn new(notifier: impl Notifier) -> Self {
        Self(Arc::new(notifier))
    }

    /// Build a shell with no platform notification effects.
    pub fn headless() -> Self {
        struct Headless;

        impl Notifier for Headless {
            fn tick(&self) {}
            fn request_redraw(&self) {}
            fn invalidate_layout(&self) {}
        }

        Self::new(Headless)
    }

    /// Begin a deferred layout/paint/tick transaction.
    ///
    /// Each kind of work is forwarded once, on flush or when the
    /// transaction goes out of scope. Independent work types are never
    /// combined because platform backends may handle them differently.
    #[must_use = "record a request with the returned transaction"]
    pub fn batch(&self) -> FrameBatch<'_> {
        FrameBatch {
            shell: self,
            pending: FrameIntent::default(),
        }
    }

    /// Immediately request a renderer tick.
    pub fn tick(&self) {
        self.0.tick();
    }

    /// Immediately request repaint.
    pub fn request_redraw(&self) {
        self.0.request_redraw();
    }

    /// Immediately invalidate window layout.
    pub fn invalidate_layout(&self) {
        self.0.invalidate_layout();
    }
}

/// Fixed-width renderer request set, with independent layout, paint and tick.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
struct FrameIntent(u8);

impl FrameIntent {
    const LAYOUT: Self = Self(1);
    const PAINT: Self = Self(2);
    const TICK: Self = Self(4);

    #[inline]
    fn add(&mut self, request: Self) {
        self.0 |= request.0;
    }

    #[inline]
    fn contains(self, request: Self) -> bool {
        self.0 & request.0 != 0
    }

    #[inline]
    fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// One TAIM renderer transaction, committed on explicit flush or drop.
///
/// A batch is window-scoped via the parent Shell. It does not allocate
/// a queue and does not change legacy immediate notification behavior.
#[must_use = "pending renderer requests are flushed when dropped"]
pub struct FrameBatch<'a> {
    shell: &'a Shell,
    pending: FrameIntent,
}

impl FrameBatch<'_> {
    /// Request a tick after the current renderer transaction.
    pub fn tick(&mut self) {
        self.pending.add(FrameIntent::TICK);
    }

    /// Request a repaint once after the current renderer transaction.
    pub fn request_redraw(&mut self) {
        self.pending.add(FrameIntent::PAINT);
    }

    /// Mark layout dirty once after the current renderer transaction.
    pub fn invalidate_layout(&mut self) {
        self.pending.add(FrameIntent::LAYOUT);
    }

    /// Publish deferred requests once and clear the transaction.
    pub fn flush(&mut self) {
        // Consume before calling platform callbacks so an early return or
        // later Drop does not replay already published notifications.
        let pending = std::mem::take(&mut self.pending);
        if pending.is_empty() {
            return;
        }

        if pending.contains(FrameIntent::LAYOUT) {
            self.shell.invalidate_layout();
        }
        if pending.contains(FrameIntent::PAINT) {
            self.shell.request_redraw();
        }
        if pending.contains(FrameIntent::TICK) {
            self.shell.tick();
        }
    }

    /// Whether any work remains to be committed.
    #[must_use]
    pub fn has_pending_requests(&self) -> bool {
        !self.pending.is_empty()
    }
}

impl Drop for FrameBatch<'_> {
    fn drop(&mut self) {
        self.flush();
    }
}

/// Platform/window notification adapter, unchanged for legacy implementors.
pub trait Notifier: Send + Sync + 'static {
    /// Request a renderer tick.
    fn tick(&self);
    /// Request repaint.
    fn request_redraw(&self);
    /// Invalidate all window layouts.
    fn invalidate_layout(&self);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct Counters {
        tick: AtomicUsize,
        paint: AtomicUsize,
        layout: AtomicUsize,
    }

    #[derive(Clone)]
    struct CountNotifier(Arc<Counters>);

    impl Notifier for CountNotifier {
        fn tick(&self) {
            self.0.tick.fetch_add(1, Ordering::Relaxed);
        }
        fn request_redraw(&self) {
            self.0.paint.fetch_add(1, Ordering::Relaxed);
        }
        fn invalidate_layout(&self) {
            self.0.layout.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn repeated_work_collapses_by_kind_on_drop() {
        let counts = Arc::new(Counters::default());
        let shell = Shell::new(CountNotifier(counts.clone()));
        {
            let mut batch = shell.batch();
            for _ in 0..100 {
                batch.invalidate_layout();
                batch.request_redraw();
                batch.tick();
            }
            assert!(batch.has_pending_requests());
        }

        assert_eq!(counts.layout.load(Ordering::Relaxed), 1);
        assert_eq!(counts.paint.load(Ordering::Relaxed), 1);
        assert_eq!(counts.tick.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn explicit_flush_is_idempotent_and_transaction_can_be_reused() {
        let counts = Arc::new(Counters::default());
        let shell = Shell::new(CountNotifier(counts.clone()));
        let mut batch = shell.batch();
        assert!(!batch.has_pending_requests());
        batch.request_redraw();
        batch.flush();
        batch.flush();
        assert!(!batch.has_pending_requests());
        batch.request_redraw();
        batch.flush();
        drop(batch);
        assert_eq!(counts.paint.load(Ordering::Relaxed), 2);
        assert_eq!(counts.layout.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn immediate_api_keeps_legacy_notification_semantics() {
        let counts = Arc::new(Counters::default());
        let shell = Shell::new(CountNotifier(counts.clone()));
        shell.tick();
        shell.request_redraw();
        shell.invalidate_layout();
        assert_eq!(counts.tick.load(Ordering::Relaxed), 1);
        assert_eq!(counts.paint.load(Ordering::Relaxed), 1);
        assert_eq!(counts.layout.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn cloned_shells_notify_shared_platform_target() {
        let counts = Arc::new(Counters::default());
        let shell = Shell::new(CountNotifier(counts.clone()));
        let clone = shell.clone();
        let mut batch = clone.batch();
        batch.tick();
        batch.invalidate_layout();
        batch.flush();
        assert_eq!(counts.tick.load(Ordering::Relaxed), 1);
        assert_eq!(counts.layout.load(Ordering::Relaxed), 1);
    }
}
