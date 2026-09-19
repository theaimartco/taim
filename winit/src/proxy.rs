use crate::futures::futures::{Future, channel::mpsc};
use crate::runtime::Action;
use crate::runtime::window;
use std::sync::Arc;

mod flow;
mod notifier;
mod sink;

use flow::Flow;

/// An event loop proxy with backpressure that implements `Sink`.
#[derive(Debug)]
pub struct Proxy<T: 'static> {
    raw: winit::event_loop::EventLoopProxy<Action<T>>,
    sender: mpsc::Sender<Action<T>>,
    notifier: mpsc::Sender<()>,
    flow: Arc<Flow>,
}

impl<T: 'static> Clone for Proxy<T> {
    fn clone(&self) -> Self {
        Self {
            raw: self.raw.clone(),
            sender: self.sender.clone(),
            notifier: self.notifier.clone(),
            flow: Arc::clone(&self.flow),
        }
    }
}

impl<T: 'static> Proxy<T> {
    const MAX_SIZE: usize = flow::EVENT_QUEUE_CAPACITY;
    const TICK_WAKE: u8 = 1 << 0;
    const REDRAW_WAKE: u8 = 1 << 1;
    const RELAYOUT_WAKE: u8 = 1 << 2;
    const RELOAD_WAKE: u8 = 1 << 3;

    /// Creates a new [`Proxy`] from an `EventLoopProxy`.
    pub fn new(
        raw: winit::event_loop::EventLoopProxy<Action<T>>,
    ) -> (Self, impl Future<Output = ()>) {
        let (notifier, processed) = mpsc::channel(1);
        let (sender, receiver) = mpsc::channel(Self::MAX_SIZE);
        let proxy = raw.clone();
        let flow = Arc::new(Flow::new());
        let worker = flow::forward(proxy, receiver, processed, Arc::clone(&flow));

        (
            Self {
                raw,
                sender,
                notifier,
                flow,
            },
            worker,
        )
    }

    /// Creates an input channel for the [`Proxy`].
    pub fn input(&self) -> mpsc::Sender<Action<T>> {
        self.sender.clone()
    }

    /// Sends a value to the event loop.
    ///
    /// This is a direct event-loop wake; callers that produce streams should
    /// prefer the [`Sink`] implementation so queue pressure propagates.
    pub fn send(&self, value: T) {
        self.send_action(Action::Output(value));
    }

    /// Sends an action to the event loop.
    ///
    /// High-frequency framework wake actions are coalesced while one matching
    /// action is already queued in Winit.
    pub fn send_action(&self, action: Action<T>) {
        match action {
            Action::Tick => self.send_once(Action::Tick, Self::TICK_WAKE),
            Action::Window(window::Action::RedrawAll) => {
                self.send_once(Action::Window(window::Action::RedrawAll), Self::REDRAW_WAKE)
            }
            Action::Window(window::Action::RelayoutAll) => self.send_once(
                Action::Window(window::Action::RelayoutAll),
                Self::RELAYOUT_WAKE,
            ),
            Action::Reload => self.send_once(Action::Reload, Self::RELOAD_WAKE),
            action => {
                let _ = self.raw.send_event(action);
            }
        }
    }

    /// Releases the coalescing bit for an action as dispatch begins.
    pub fn acknowledge(&self, action: &Action<T>) {
        let wake = match action {
            Action::Tick => Self::TICK_WAKE,
            Action::Window(window::Action::RedrawAll) => Self::REDRAW_WAKE,
            Action::Window(window::Action::RelayoutAll) => Self::RELAYOUT_WAKE,
            Action::Reload => Self::RELOAD_WAKE,
            _ => return,
        };

        self.flow.acknowledge(wake);
    }

    /// Frees an amount of slots for additional messages to be queued in
    /// this [`Proxy`].
    pub fn free_slots(&mut self, amount: usize) {
        if amount == 0 {
            return;
        }

        self.flow.release(amount);
        let _ = self.notifier.try_send(());
    }

    fn send_once(&self, action: Action<T>, wake: u8) {
        if !self.flow.claim_wake(wake) {
            return;
        }

        if self.raw.send_event(action).is_err() {
            self.flow.acknowledge(wake);
        }
    }
}
