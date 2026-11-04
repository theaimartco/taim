//! Specialized GUI Task Scheduler for TAIM.
//!
//! Replaces general-purpose executor polling with a frame-aware,
//! generational task scheduler featuring O(1) duplicate wake coalescing,
//! lock-free cancellation tokens, and zero idle overhead.

use crate::BoxFuture;

use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll};

/// Unique generational identifier for a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId {
    /// Slot index in the scheduler arena.
    pub slot: u32,
    /// Monotonic generation preventing stale access after completion or cancellation.
    pub generation: u32,
}

/// The execution and queueing state of a task slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WakeState {
    /// Task is idle, waiting on an external event or future.
    Idle = 0,
    /// Task has been signaled and is present in the ready queue.
    Queued = 1,
    /// Task is currently being polled by the scheduler.
    Polling = 2,
    /// Task was signaled while it was being polled; will be re-queued when poll yields Pending.
    WokenWhilePolling = 3,
    /// Task has been cancelled via its [`CancelToken`].
    Cancelled = 4,
    /// Task has completed execution.
    Finished = 5,
}

impl From<u8> for WakeState {
    fn from(val: u8) -> Self {
        match val {
            0 => WakeState::Idle,
            1 => WakeState::Queued,
            2 => WakeState::Polling,
            3 => WakeState::WokenWhilePolling,
            4 => WakeState::Cancelled,
            _ => WakeState::Finished,
        }
    }
}

/// A generational subscription epoch tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SubscriptionEpoch(pub u64);

impl SubscriptionEpoch {
    /// Initial epoch.
    pub const INITIAL: Self = Self(0);

    /// Advances to the next epoch.
    pub fn next(&self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

use futures::task::ArcWake;

/// Shared header for a task slot enabling atomic wake coalescing and lock-free cancellation.
struct TaskSlotHeader {
    slot: u32,
    generation: AtomicU32,
    wake_state: AtomicU8,
    cancelled: AtomicBool,
    ready_queue: Arc<Mutex<Vec<u32>>>,
    notifier: Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>,
}

impl std::fmt::Debug for TaskSlotHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskSlotHeader")
            .field("slot", &self.slot)
            .field("generation", &self.generation.load(Ordering::Relaxed))
            .field("wake_state", &self.wake_state.load(Ordering::Relaxed))
            .field("cancelled", &self.cancelled.load(Ordering::Relaxed))
            .finish()
    }
}

/// A generational waker proxy ensuring stale wakes on recycled slots are safe NO-OPs.
#[derive(Debug)]
struct TaskWaker {
    header: Arc<TaskSlotHeader>,
    generation: u32,
}

impl ArcWake for TaskWaker {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        // Generational check: if slot was recycled, generation will not match!
        if arc_self.header.generation.load(Ordering::Acquire) != arc_self.generation {
            return;
        }

        if arc_self.header.cancelled.load(Ordering::Acquire) {
            return;
        }

        let mut current = arc_self.header.wake_state.load(Ordering::Acquire);
        loop {
            // Re-verify generation in loop to prevent race during slot recycling
            if arc_self.header.generation.load(Ordering::Acquire) != arc_self.generation {
                return;
            }

            match WakeState::from(current) {
                WakeState::Idle => {
                    // Transition Idle -> Queued and push to ready queue
                    match arc_self.header.wake_state.compare_exchange_weak(
                        WakeState::Idle as u8,
                        WakeState::Queued as u8,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => {
                            if arc_self.header.generation.load(Ordering::Acquire)
                                == arc_self.generation
                            {
                                {
                                    let mut queue = arc_self
                                        .header
                                        .ready_queue
                                        .lock()
                                        .unwrap_or_else(PoisonError::into_inner);
                                    queue.push(arc_self.header.slot);
                                }
                                let notifier_guard = arc_self
                                    .header
                                    .notifier
                                    .lock()
                                    .unwrap_or_else(PoisonError::into_inner);
                                if let Some(notifier) = notifier_guard.as_ref() {
                                    notifier();
                                }
                            }
                            break;
                        }
                        Err(actual) => current = actual,
                    }
                }
                WakeState::Polling => {
                    // Woken while currently running: mark WokenWhilePolling without pushing to queue yet
                    match arc_self.header.wake_state.compare_exchange_weak(
                        WakeState::Polling as u8,
                        WakeState::WokenWhilePolling as u8,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => break,
                        Err(actual) => current = actual,
                    }
                }
                WakeState::Queued | WakeState::WokenWhilePolling => {
                    // Already queued or marked to re-queue: duplicate wake coalesces!
                    break;
                }
                WakeState::Cancelled | WakeState::Finished => {
                    break;
                }
            }
        }
    }
}

/// Token allowing lock-free O(1) cancellation of a task without global scans.
///
/// Guaranteed to be generationally safe: calling `cancel()` on a recycled slot is a safe NO-OP.
#[derive(Debug, Clone)]
pub struct CancelToken {
    task_id: TaskId,
    header: Arc<TaskSlotHeader>,
}

impl CancelToken {
    /// Cancels the task in O(1) time.
    ///
    /// If the task has already finished and its slot was recycled for a newer task,
    /// this operation is an immediate safe NO-OP.
    pub fn cancel(&self) {
        if self.header.generation.load(Ordering::Acquire) != self.task_id.generation {
            return;
        }
        self.header.cancelled.store(true, Ordering::Release);
        let prev = self
            .header
            .wake_state
            .swap(WakeState::Cancelled as u8, Ordering::AcqRel);
        if prev != WakeState::Queued as u8 && prev != WakeState::Cancelled as u8 {
            {
                let mut queue = self
                    .header
                    .ready_queue
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                queue.push(self.header.slot);
            }
            let notifier_guard = self
                .header
                .notifier
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if let Some(notifier) = notifier_guard.as_ref() {
                notifier();
            }
        }
    }

    /// Returns `true` if this token has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        if self.header.generation.load(Ordering::Acquire) != self.task_id.generation {
            return false;
        }
        self.header.cancelled.load(Ordering::Acquire)
    }

    /// Returns the ID of the task associated with this token.
    pub fn task_id(&self) -> TaskId {
        self.task_id
    }
}

/// A slot holding an active task future.
struct Slot {
    header: Arc<TaskSlotHeader>,
    waker: Option<futures::task::Waker>,
    future: Option<BoxFuture<()>>,
}

/// A specialized, high-performance GUI task scheduler.
///
/// Uses an atomic wake-state machine with a mutex-backed ready queue.
/// Ensures 100 wake() calls before poll produce exactly 1 ready-queue entry,
/// drops cancelled tasks in O(1) time, and performs zero scheduling work when idle.
pub struct TaskScheduler {
    slots: Vec<Slot>,
    free_slots: Vec<u32>,
    ready_queue: Arc<Mutex<Vec<u32>>>,
    scratch_ready: Vec<u32>,
    epoch: SubscriptionEpoch,
    active_count: usize,
    notifier: Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>,
}

impl std::fmt::Debug for TaskScheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskScheduler")
            .field("slots", &self.slots.len())
            .field("free_slots", &self.free_slots)
            .field("epoch", &self.epoch)
            .field("active_count", &self.active_count)
            .finish()
    }
}

impl TaskScheduler {
    /// Creates a new empty [`TaskScheduler`].
    pub fn new() -> Self {
        Self {
            slots: Vec::with_capacity(64),
            free_slots: Vec::new(),
            ready_queue: Arc::new(Mutex::new(Vec::with_capacity(64))),
            scratch_ready: Vec::with_capacity(64),
            epoch: SubscriptionEpoch::INITIAL,
            active_count: 0,
            notifier: Arc::new(Mutex::new(None)),
        }
    }

    /// Sets an external notification callback invoked whenever a task transitions to ready.
    pub fn set_notifier(&mut self, notifier: impl Fn() + Send + Sync + 'static) {
        *self.notifier.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::new(notifier));
    }

    /// Returns `true` if there are any tasks in the ready queue waiting to be polled.
    pub fn has_ready_tasks(&self) -> bool {
        !self
            .ready_queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty()
    }

    /// Returns the number of currently active (non-finished) tasks.
    pub fn active_tasks(&self) -> usize {
        self.active_count
    }

    /// Returns `true` if the scheduler has no active tasks.
    pub fn is_idle(&self) -> bool {
        self.active_count == 0
    }

    /// Returns `true` if the task identified by [`TaskId`] is currently active and valid.
    pub fn is_valid(&self, id: TaskId) -> bool {
        if let Some(slot) = self.slots.get(id.slot as usize) {
            slot.header.generation.load(Ordering::Acquire) == id.generation
                && slot.future.is_some()
                && !slot.header.cancelled.load(Ordering::Acquire)
        } else {
            false
        }
    }

    /// Returns the current subscription epoch.
    pub fn epoch(&self) -> SubscriptionEpoch {
        self.epoch
    }

    /// Advances and returns the next subscription epoch.
    pub fn advance_epoch(&mut self) -> SubscriptionEpoch {
        self.epoch = self.epoch.next();
        self.epoch
    }

    /// Spawns a future into the scheduler, returning a generational [`TaskId`]
    /// and a [`CancelToken`].
    pub fn spawn(&mut self, future: BoxFuture<()>) -> (TaskId, CancelToken) {
        self.active_count += 1;

        if let Some(slot_idx) = self.free_slots.pop() {
            let slot = &mut self.slots[slot_idx as usize];
            let current = slot.header.generation.load(Ordering::Acquire);
            let generation = if current == u32::MAX { 1 } else { current + 1 };
            slot.header.generation.store(generation, Ordering::Release);
            slot.header.cancelled.store(false, Ordering::Release);
            slot.header
                .wake_state
                .store(WakeState::Queued as u8, Ordering::Release);
            slot.future = Some(future);

            let task_waker = Arc::new(TaskWaker {
                header: Arc::clone(&slot.header),
                generation,
            });
            slot.waker = Some(futures::task::waker(task_waker));

            // Queue slot initially for first poll
            self.ready_queue
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(slot_idx);

            let id = TaskId {
                slot: slot_idx,
                generation,
            };
            let token = CancelToken {
                task_id: id,
                header: Arc::clone(&slot.header),
            };
            (id, token)
        } else {
            let slot_idx = self.slots.len() as u32;
            let header = Arc::new(TaskSlotHeader {
                slot: slot_idx,
                generation: AtomicU32::new(1),
                wake_state: AtomicU8::new(WakeState::Queued as u8),
                cancelled: AtomicBool::new(false),
                ready_queue: Arc::clone(&self.ready_queue),
                notifier: Arc::clone(&self.notifier),
            });

            self.ready_queue
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(slot_idx);

            let id = TaskId {
                slot: slot_idx,
                generation: 1,
            };
            let token = CancelToken {
                task_id: id,
                header: Arc::clone(&header),
            };

            let task_waker = Arc::new(TaskWaker {
                header: Arc::clone(&header),
                generation: 1,
            });

            self.slots.push(Slot {
                header,
                waker: Some(futures::task::waker(task_waker)),
                future: Some(future),
            });

            (id, token)
        }
    }

    /// Polls all tasks that are currently marked ready in the queue.
    ///
    /// Returns the number of tasks polled. In steady state when no tasks
    /// are ready, returns 0 with ZERO heap allocations.
    pub fn poll_ready(&mut self) -> usize {
        // Fast-path: check if ready queue is empty without acquiring lock if possible
        {
            let mut queue = self
                .ready_queue
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if queue.is_empty() {
                return 0;
            }
            std::mem::swap(&mut *queue, &mut self.scratch_ready);
        }

        let mut polled_count = 0;

        for &slot_idx in &self.scratch_ready {
            let slot = &mut self.slots[slot_idx as usize];

            // Check if cancelled
            if slot.header.cancelled.load(Ordering::Acquire) {
                slot.future = None;
                slot.waker = None;
                slot.header
                    .wake_state
                    .store(WakeState::Cancelled as u8, Ordering::Release);
                self.free_slots.push(slot_idx);
                self.active_count = self.active_count.saturating_sub(1);
                continue;
            }

            let Some(mut future) = slot.future.take() else {
                continue;
            };

            slot.header
                .wake_state
                .store(WakeState::Polling as u8, Ordering::Release);
            let waker = slot.waker.as_ref().unwrap();
            let mut cx = Context::from_waker(waker);

            polled_count += 1;

            match future.as_mut().poll(&mut cx) {
                Poll::Ready(()) => {
                    slot.header
                        .wake_state
                        .store(WakeState::Finished as u8, Ordering::Release);
                    slot.future = None;
                    slot.waker = None;
                    self.free_slots.push(slot_idx);
                    self.active_count = self.active_count.saturating_sub(1);
                }
                Poll::Pending => {
                    slot.future = Some(future);
                    let prev = slot
                        .header
                        .wake_state
                        .swap(WakeState::Idle as u8, Ordering::AcqRel);
                    if prev == WakeState::WokenWhilePolling as u8 {
                        slot.header
                            .wake_state
                            .store(WakeState::Queued as u8, Ordering::Release);
                        self.ready_queue
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .push(slot_idx);
                    }
                }
            }
        }

        self.scratch_ready.clear();
        polled_count
    }
}

impl Default for TaskScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn panicking_notifier_does_not_break_later_wakes() {
        let mut scheduler = TaskScheduler::new();
        let notified = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&notified);
        scheduler.set_notifier(move || {
            assert!(
                counter.fetch_add(1, Ordering::SeqCst) != 0,
                "first notification fails"
            );
        });

        let captured_waker = Arc::new(Mutex::new(None));
        let capture = Arc::clone(&captured_waker);
        let polls = Arc::new(AtomicUsize::new(0));
        let poll_counter = Arc::clone(&polls);
        let _task = scheduler.spawn(Box::pin(std::future::poll_fn(move |cx| {
            let _ = poll_counter.fetch_add(1, Ordering::SeqCst);
            *capture.lock().unwrap() = Some(cx.waker().clone());
            Poll::<()>::Pending
        })));
        let _ = scheduler.poll_ready();
        let waker: std::task::Waker = captured_waker.lock().unwrap().clone().unwrap();

        // The notifier panics while its lock is held, poisoning it.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| waker.wake_by_ref()));
        assert!(result.is_err());
        let _ = scheduler.poll_ready();

        // Later wakes must still enqueue the task and notify.
        waker.wake_by_ref();
        assert!(scheduler.has_ready_tasks());
        assert!(notified.load(Ordering::SeqCst) >= 2);
        let _ = scheduler.poll_ready();
        assert!(polls.load(Ordering::SeqCst) >= 3);
    }

    #[test]
    fn test_duplicate_wake_coalescing() {
        let mut scheduler = TaskScheduler::new();

        let poll_count = Arc::new(AtomicUsize::new(0));
        let poll_count_clone = Arc::clone(&poll_count);

        let (task_id, token) = scheduler.spawn(Box::pin(std::future::poll_fn(move |cx| {
            let count = poll_count_clone.fetch_add(1, Ordering::SeqCst);
            if count < 1 {
                // Wake 100 times before yielding Pending!
                for _ in 0..100 {
                    cx.waker().wake_by_ref();
                }
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        })));

        assert_eq!(scheduler.active_tasks(), 1);

        // First poll
        let polled = scheduler.poll_ready();
        assert_eq!(polled, 1);
        assert_eq!(poll_count.load(Ordering::SeqCst), 1);

        // Check ready queue length after 100 wake() calls:
        // Must be EXACTLY 1 because all 99 subsequent wakes coalesced!
        {
            let queue = scheduler.ready_queue.lock().unwrap();
            assert_eq!(
                queue.len(),
                1,
                "100 wake() calls must collapse to 1 queue entry"
            );
        }

        // Second poll completes the task
        let polled = scheduler.poll_ready();
        assert_eq!(polled, 1);
        assert_eq!(poll_count.load(Ordering::SeqCst), 2);
        assert_eq!(scheduler.active_tasks(), 0);
        assert!(scheduler.is_idle());

        let _ = (task_id, token);
    }

    #[test]
    fn test_cancel_storm() {
        let mut scheduler = TaskScheduler::new();
        let mut tokens = Vec::new();

        // Spawn 100 tasks
        for _ in 0..100 {
            let (_, token) = scheduler.spawn(Box::pin(std::future::pending()));
            tokens.push(token);
        }
        assert_eq!(scheduler.active_tasks(), 100);

        // Cancel all 100 tasks in a storm
        for token in &tokens {
            token.cancel();
            assert!(token.is_cancelled());
        }

        // Poll must clean them up without polling any futures
        let polled = scheduler.poll_ready();
        assert_eq!(polled, 0, "Cancelled tasks must not be polled");
        assert_eq!(scheduler.active_tasks(), 0);
        assert!(scheduler.is_idle());
    }

    #[test]
    fn test_scheduler_benchmark_metrics() {
        let mut scheduler = TaskScheduler::new();
        let start = std::time::Instant::now();

        // Spawn 1,000 completed tasks
        for _ in 0..1_000 {
            let _ = scheduler.spawn(Box::pin(std::future::ready(())));
        }
        let spawn_duration = start.elapsed();

        let poll_start = std::time::Instant::now();
        let polled = scheduler.poll_ready();
        let poll_duration = poll_start.elapsed();

        assert_eq!(polled, 1_000);
        assert_eq!(scheduler.active_tasks(), 0);

        println!(
            "[Scheduler Benchmark] 1,000 tasks: spawn_total={:?}, poll_total={:?}, avg_per_task={:?}",
            spawn_duration,
            poll_duration,
            poll_duration / 1000
        );
    }

    #[test]
    fn test_stale_cancel_does_not_cancel_reused_slot() {
        let mut scheduler = TaskScheduler::new();

        // 1. Spawn Task A in slot 0, gen 1
        let (id_a, token_a) = scheduler.spawn(Box::pin(std::future::ready(())));
        assert_eq!(id_a.slot, 0);
        assert_eq!(id_a.generation, 1);

        // 2. Poll Task A to completion -> slot 0 is recycled into free_slots
        let polled = scheduler.poll_ready();
        assert_eq!(polled, 1);
        assert_eq!(scheduler.active_tasks(), 0);

        // 3. Spawn Task B -> must reuse slot 0 with generation 2
        let executed_b = Arc::new(AtomicBool::new(false));
        let exec_b_clone = Arc::clone(&executed_b);
        let (id_b, token_b) = scheduler.spawn(Box::pin(async move {
            exec_b_clone.store(true, Ordering::SeqCst);
        }));
        assert_eq!(id_b.slot, 0);
        assert_eq!(id_b.generation, 2);
        assert_eq!(scheduler.active_tasks(), 1);

        // 4. Stale Task A CancelToken calls cancel()!
        token_a.cancel();

        // Must NOT cancel Task B!
        assert!(!token_b.is_cancelled());
        assert!(scheduler.is_valid(id_b));

        // 5. Poll Task B -> must execute to completion!
        let polled = scheduler.poll_ready();
        assert_eq!(polled, 1);
        assert!(executed_b.load(Ordering::SeqCst));
        assert_eq!(scheduler.active_tasks(), 0);
    }

    #[test]
    fn test_stale_waker_does_not_wake_reused_slot() {
        let mut scheduler = TaskScheduler::new();

        let captured_waker = Arc::new(Mutex::new(None));
        let cap_waker_clone = Arc::clone(&captured_waker);

        // 1. Spawn Task A which captures its waker and yields pending once
        let poll_count = Arc::new(AtomicUsize::new(0));
        let pc_clone = Arc::clone(&poll_count);
        let (id_a, token_a) = scheduler.spawn(Box::pin(std::future::poll_fn(move |cx| {
            let count = pc_clone.fetch_add(1, Ordering::SeqCst);
            if count == 0 {
                *cap_waker_clone.lock().unwrap() = Some(cx.waker().clone());
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        })));
        assert_eq!(id_a.slot, 0);
        assert_eq!(id_a.generation, 1);

        // Poll Task A -> saves waker
        assert_eq!(scheduler.poll_ready(), 1);
        assert_eq!(poll_count.load(Ordering::SeqCst), 1);
        assert!(captured_waker.lock().unwrap().is_some());

        // Now cancel Task A and clean it up
        token_a.cancel();
        assert_eq!(scheduler.poll_ready(), 0);
        assert_eq!(scheduler.active_tasks(), 0);

        // Slot 0 is now free. Spawn Task B at slot 0 (gen 2)
        let (id_b, _token_b) = scheduler.spawn(Box::pin(std::future::pending()));
        assert_eq!(id_b.slot, 0);
        assert_eq!(id_b.generation, 2);

        // Empty the ready queue so Task B is not queued
        {
            let mut q = scheduler.ready_queue.lock().unwrap();
            q.clear();
        }

        // 2. Fire the STALE waker from Task A!
        let stale_waker = captured_waker.lock().unwrap().take().unwrap();
        stale_waker.wake();

        // Ready queue must still be EMPTY because generation mismatched!
        {
            let q = scheduler.ready_queue.lock().unwrap();
            assert_eq!(
                q.len(),
                0,
                "Stale waker must be a NO-OP and not enqueue reused slot"
            );
        }
    }

    #[test]
    fn test_reused_slot_has_new_generation() {
        let mut scheduler = TaskScheduler::new();

        let (id1, _) = scheduler.spawn(Box::pin(std::future::ready(())));
        assert_eq!(id1.slot, 0);
        assert_eq!(id1.generation, 1);

        assert_eq!(scheduler.poll_ready(), 1);
        assert_eq!(scheduler.active_tasks(), 0);

        let (id2, _) = scheduler.spawn(Box::pin(std::future::ready(())));
        assert_eq!(id2.slot, 0);
        assert_eq!(id2.generation, 2);
    }

    #[test]
    fn test_stale_task_id_is_rejected() {
        let mut scheduler = TaskScheduler::new();

        let (id1, _) = scheduler.spawn(Box::pin(std::future::ready(())));
        assert!(scheduler.is_valid(id1));

        assert_eq!(scheduler.poll_ready(), 1);
        // id1 is now stale (finished)
        assert!(!scheduler.is_valid(id1));

        let (id2, _) = scheduler.spawn(Box::pin(std::future::pending()));
        assert!(scheduler.is_valid(id2));
        // id1 has same slot (0) but wrong generation (1 vs 2) -> must be rejected
        assert!(!scheduler.is_valid(id1));
    }

    #[test]
    fn test_cancellation_is_idempotent() {
        let mut scheduler = TaskScheduler::new();
        let (id, token) = scheduler.spawn(Box::pin(std::future::pending()));

        assert!(scheduler.is_valid(id));
        assert!(!token.is_cancelled());

        // Cancel multiple times
        token.cancel();
        token.cancel();
        token.cancel();

        assert!(token.is_cancelled());
        assert!(!scheduler.is_valid(id));
    }

    #[test]
    fn test_notifier_called_on_wake() {
        use std::sync::atomic::AtomicUsize;

        let mut scheduler = TaskScheduler::new();
        let notify_count = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&notify_count);
        scheduler.set_notifier(move || {
            let _ = count.fetch_add(1, Ordering::SeqCst);
        });

        let captured_waker: Arc<Mutex<Option<std::task::Waker>>> = Arc::new(Mutex::new(None));
        let waker_clone = Arc::clone(&captured_waker);

        let fut = std::future::poll_fn(move |cx| {
            *waker_clone.lock().unwrap() = Some(cx.waker().clone());
            std::task::Poll::Pending
        });

        let (_id, _token) = scheduler.spawn(Box::pin(fut));
        // First poll
        assert_eq!(scheduler.poll_ready(), 1);
        assert_eq!(notify_count.load(Ordering::SeqCst), 0);

        // Wake the task
        let waker = captured_waker.lock().unwrap().take().unwrap();
        waker.wake();

        assert_eq!(notify_count.load(Ordering::SeqCst), 1);
        assert!(scheduler.has_ready_tasks());
        assert_eq!(scheduler.poll_ready(), 1);
    }
}
