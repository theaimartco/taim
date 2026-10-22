//! The handle the rest of the framework uses to reach the event loop.
use crate::futures::futures::{Future, channel::mpsc};
use crate::runtime::Action;
use crate::runtime::window;

use std::sync::Arc;

mod flow;
mod notifier;
mod sink;

use flow::Flow;

/// A cloneable, thread-safe way to hand [`Action`]s to the event loop.
///
/// There are two paths in:
///
/// - [`send_action`](Self::send_action) wakes the event loop directly. The
///   four framework "pulse" actions (tick, redraw-all, relayout-all, reload)
///   are coalesced: while one is queued, sending it again is free.
/// - The [`Sink`](futures::Sink) implementation is bounded. Streams feeding
///   it are back-pressured until the shell reports, via
///   [`free_slots`](Self::free_slots), that it consumed earlier actions.
#[derive(Debug)]
pub struct Proxy<T: 'static> {
    event_loop: winit::event_loop::EventLoopProxy<Action<T>>,
    /// Bounded input of the forwarding task.
    queue: mpsc::Sender<Action<T>>,
    /// Pokes the forwarding task when slots are freed.
    consumed: mpsc::Sender<()>,
    flow: Arc<Flow>,
}

/// One coalescing bit per pulse action.
#[derive(Clone, Copy)]
#[repr(u8)]
enum Pulse {
    Tick = 1 << 0,
    RedrawAll = 1 << 1,
    RelayoutAll = 1 << 2,
    Reload = 1 << 3,
}

impl Pulse {
    fn of<T>(action: &Action<T>) -> Option<Self> {
        Some(match action {
            Action::Tick => Self::Tick,
            Action::Window(window::Action::RedrawAll) => Self::RedrawAll,
            Action::Window(window::Action::RelayoutAll) => Self::RelayoutAll,
            Action::Reload => Self::Reload,
            _ => return None,
        })
    }

    fn bit(self) -> u8 {
        self as u8
    }
}

impl<T: 'static> Clone for Proxy<T> {
    fn clone(&self) -> Self {
        Self {
            event_loop: self.event_loop.clone(),
            queue: self.queue.clone(),
            consumed: self.consumed.clone(),
            flow: Arc::clone(&self.flow),
        }
    }
}

impl<T: 'static> Proxy<T> {
    /// Wraps an event-loop proxy. The returned future forwards the bounded
    /// queue into the event loop and must be spawned on the executor.
    pub fn new(
        event_loop: winit::event_loop::EventLoopProxy<Action<T>>,
    ) -> (Self, impl Future<Output = ()>) {
        let (consumed, consumed_signal) = mpsc::channel(1);
        let (queue, queued) = mpsc::channel(flow::EVENT_QUEUE_CAPACITY);
        let flow = Arc::new(Flow::new());
        let forwarder = flow::forward(
            event_loop.clone(),
            queued,
            consumed_signal,
            Arc::clone(&flow),
        );

        (
            Self {
                event_loop,
                queue,
                consumed,
                flow,
            },
            forwarder,
        )
    }

    /// A bounded sender into the same queue the [`Sink`](futures::Sink)
    /// implementation uses.
    pub fn input(&self) -> mpsc::Sender<Action<T>> {
        self.queue.clone()
    }

    /// Sends an application message, waking the event loop directly.
    pub fn send(&self, message: T) {
        self.send_action(Action::Output(message));
    }

    /// Sends an action, waking the event loop directly.
    pub fn send_action(&self, action: Action<T>) {
        match Pulse::of(&action) {
            Some(pulse) => self.pulse(action, pulse),
            None => {
                let _ = self.event_loop.send_event(action);
            }
        }
    }

    /// Re-arms a pulse once the shell starts handling it.
    pub fn acknowledge(&self, action: &Action<T>) {
        if let Some(pulse) = Pulse::of(action) {
            self.flow.acknowledge(pulse.bit());
        }
    }

    /// Reports that the shell consumed `amount` queued actions, letting the
    /// forwarder admit that many more.
    pub fn free_slots(&mut self, amount: usize) {
        if amount == 0 {
            return;
        }

        self.flow.release(amount);
        let _ = self.consumed.try_send(());
    }

    fn pulse(&self, action: Action<T>, pulse: Pulse) {
        if !self.flow.claim_wake(pulse.bit()) {
            return;
        }

        if self.event_loop.send_event(action).is_err() {
            self.flow.acknowledge(pulse.bit());
        }
    }
}
