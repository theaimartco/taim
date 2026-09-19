use crate::futures::futures::{FutureExt, StreamExt, channel::mpsc, pin_mut, select};
use crate::runtime::Action;

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

pub(super) const EVENT_QUEUE_CAPACITY: usize = 100;

/// Lock-free accounting shared by all clones of an event-loop proxy.
#[derive(Debug)]
pub(super) struct Flow {
    released: AtomicUsize,
    pending_wakes: AtomicU8,
}

impl Flow {
    pub(super) const fn new() -> Self {
        Self {
            released: AtomicUsize::new(0),
            pending_wakes: AtomicU8::new(0),
        }
    }

    /// Claims a coalesced wake bit. False means an equivalent wake is already
    /// queued and no event-loop syscall is needed.
    #[inline]
    pub(super) fn claim_wake(&self, wake: u8) -> bool {
        self.pending_wakes.fetch_or(wake, Ordering::AcqRel) & wake == 0
    }

    #[inline]
    pub(super) fn acknowledge(&self, wake: u8) {
        let _ = self.pending_wakes.fetch_and(!wake, Ordering::Release);
    }

    pub(super) fn release(&self, amount: usize) {
        let _ = self
            .released
            .fetch_update(Ordering::Release, Ordering::Relaxed, |released| {
                Some(released.saturating_add(amount))
            });
    }

    #[inline]
    fn drain_released(&self, in_flight: &mut usize) {
        let amount = self.released.swap(0, Ordering::Acquire);
        *in_flight = in_flight.saturating_sub(amount);
    }
}

/// Moves bounded stream output into Winit while respecting the number of
/// events the runtime has acknowledged. Channel shutdown is an ordinary exit,
/// never a `select_next_some` panic.
pub(super) async fn forward<T: 'static>(
    proxy: winit::event_loop::EventLoopProxy<Action<T>>,
    mut incoming: mpsc::Receiver<Action<T>>,
    mut processed: mpsc::Receiver<()>,
    flow: Arc<Flow>,
) {
    let mut in_flight = 0usize;

    loop {
        if in_flight >= EVENT_QUEUE_CAPACITY {
            if processed.next().await.is_none() {
                break;
            }

            flow.drain_released(&mut in_flight);
            continue;
        }

        let next_message = incoming.next().fuse();
        let next_release = processed.next().fuse();
        pin_mut!(next_message, next_release);

        select! {
            message = next_message => {
                let Some(message) = message else {
                    break;
                };

                if proxy.send_event(message).is_err() {
                    break;
                }

                in_flight += 1;
            },
            released = next_release => {
                if released.is_none() {
                    break;
                }
                flow.drain_released(&mut in_flight);
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wake_claims_are_coalesced_until_acknowledged() {
        let flow = Flow::new();

        assert!(flow.claim_wake(0b0001));
        assert!(!flow.claim_wake(0b0001));
        assert!(flow.claim_wake(0b0010));

        flow.acknowledge(0b0001);
        assert!(flow.claim_wake(0b0001));
        assert!(!flow.claim_wake(0b0010));
    }

    #[test]
    fn released_slots_saturate_and_drain_once() {
        let flow = Flow::new();
        let mut in_flight = 4;

        flow.release(usize::MAX);
        flow.release(4);
        flow.drain_released(&mut in_flight);
        assert_eq!(in_flight, 0);

        in_flight = 3;
        flow.drain_released(&mut in_flight);
        assert_eq!(in_flight, 3);
    }
}
