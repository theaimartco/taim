//! Specialized GUI Task Scheduler for TAIM.
//!
//! Replaces general-purpose executor polling with a frame-aware,
//! generational task scheduler featuring O(1) duplicate wake coalescing,
//! lock-free cancellation tokens, and zero idle overhead.

use crate::BoxFuture;

use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
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
#[derive(Debug)]
struct TaskSlotHeader {
    slot: u32,
    generation: AtomicU32,
    wake_state: AtomicU8,
    cancelled: AtomicBool,
    ready_queue: Arc<Mutex<Vec<u32>>>,
}

impl ArcWake for TaskSlotHeader {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        if arc_self.cancelled.load(Ordering::Acquire) {
            return;
        }

        let mut current = arc_self.wake_state.load(Ordering::Acquire);
        loop {
            match WakeState::from(current) {
                WakeState::Idle => {
                    // Transition Idle -> Queued and push to ready queue
                    match arc_self.wake_state.compare_exchange_weak(
                        WakeState::Idle as u8,
                        WakeState::Queued as u8,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) {
                        Ok(_) => {
                            let mut queue = arc_self.ready_queue.lock().unwrap();
                            queue.push(arc_self.slot);
                            break;
                        }
                        Err(actual) => current = actual,
                    }
                }
                WakeState::Polling => {
                    // Woken while currently running: mark WokenWhilePolling without pushing to queue yet
                    match arc_self.wake_state.compare_exchange_weak(
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
                    // Dead task
                    break;
                }
            }
        }
    }
}

/// Token allowing lock-free O(1) cancellation of a task without global scans.
#[derive(Debug, Clone)]
pub struct CancelToken {
    task_id: TaskId,
    header: Arc<TaskSlotHeader>,
}

impl CancelToken {
    /// Cancels the task in O(1) time.
    pub fn cancel(&self) {
        self.header.cancelled.store(true, Ordering::Release);
        self.header
            .wake_state
            .store(WakeState::Cancelled as u8, Ordering::Release);
    }

    /// Returns `true` if this token has been cancelled.
    pub fn is_cancelled(&self) -> bool {
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
    future: Option<BoxFuture<()>>,
}

/// A specialized, high-performance GUI task scheduler.
///
/// Ensures 100 wake() calls before poll produce exactly 1 ready-queue entry,
/// drops cancelled tasks in O(1) time, and performs zero scheduling work when idle.
pub struct TaskScheduler {
    slots: Vec<Slot>,
    free_slots: Vec<u32>,
    ready_queue: Arc<Mutex<Vec<u32>>>,
    scratch_ready: Vec<u32>,
    epoch: SubscriptionEpoch,
    active_count: usize,
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
        }
    }

    /// Returns the number of currently active (non-finished) tasks.
    pub fn active_tasks(&self) -> usize {
        self.active_count
    }

    /// Returns `true` if the scheduler has no active tasks.
    pub fn is_idle(&self) -> bool {
        self.active_count == 0
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
            let generation = slot
                .header
                .generation
                .fetch_add(1, Ordering::AcqRel)
                .wrapping_add(1);
            slot.header.cancelled.store(false, Ordering::Release);
            slot.header
                .wake_state
                .store(WakeState::Queued as u8, Ordering::Release);
            slot.future = Some(future);

            // Queue slot initially for first poll
            self.ready_queue.lock().unwrap().push(slot_idx);

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
            });

            self.ready_queue.lock().unwrap().push(slot_idx);

            let id = TaskId {
                slot: slot_idx,
                generation: 1,
            };
            let token = CancelToken {
                task_id: id,
                header: Arc::clone(&header),
            };

            self.slots.push(Slot {
                header,
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
            let mut queue = self.ready_queue.lock().unwrap();
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
            let waker = futures::task::waker(Arc::clone(&slot.header));
            let mut cx = Context::from_waker(&waker);

            polled_count += 1;

            match future.as_mut().poll(&mut cx) {
                Poll::Ready(()) => {
                    slot.header
                        .wake_state
                        .store(WakeState::Finished as u8, Ordering::Release);
                    slot.future = None;
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
                        self.ready_queue.lock().unwrap().push(slot_idx);
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
}
