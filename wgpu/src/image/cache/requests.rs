use super::worker::{self, Worker};
use super::{Cache, Callback, CallbackList};
use crate::core;
use crate::graphics::Shell;

use rustc_hash::FxHashMap;
use std::collections::VecDeque;

pub(super) enum State {
    Decode {
        wake: fn(&Shell),
    },
    Upload {
        pixels: worker::Pixels,
        wake: fn(&Shell),
    },
    InFlight,
}

pub(super) struct Request {
    ticket: u64,
    handle: core::image::Handle,
    state: State,
    pub(super) callbacks: CallbackList,
    seen_in_frame: u64,
}

/// Deduplicates image work while keeping ready-to-dispatch requests in FIFO
/// order. The queue means completion handling no longer scans every in-flight
/// request just to find the next item the bounded worker can accept.
pub(super) struct Queue {
    entries: FxHashMap<core::image::Id, Request>,
    waiting: VecDeque<(core::image::Id, u64)>,
    next_ticket: u64,
    frame: u64,
}

impl Default for Queue {
    fn default() -> Self {
        Self {
            entries: FxHashMap::default(),
            waiting: VecDeque::new(),
            next_ticket: 1,
            frame: 1,
        }
    }
}

impl Queue {
    pub(super) fn attach_callback(
        &mut self,
        id: core::image::Id,
        callback: Callback,
    ) -> Result<(), Callback> {
        let Some(request) = self.entries.get_mut(&id) else {
            return Err(callback);
        };

        request.seen_in_frame = self.frame;
        request.callbacks.push(callback);
        Ok(())
    }

    pub(super) fn touch(&mut self, id: core::image::Id) -> bool {
        let Some(request) = self.entries.get_mut(&id) else {
            return false;
        };

        request.seen_in_frame = self.frame;
        true
    }

    pub(super) fn take_callbacks(&mut self, id: core::image::Id) -> CallbackList {
        self.entries
            .remove(&id)
            .map(|request| request.callbacks)
            .unwrap_or_default()
    }

    pub(super) fn enqueue(
        &mut self,
        handle: &core::image::Handle,
        state: State,
        callback: Option<Callback>,
    ) {
        let id = handle.id();

        if let Some(request) = self.entries.get_mut(&id) {
            request.seen_in_frame = self.frame;
            request.callbacks.extend(callback);
            return;
        }

        let ticket = self.next_ticket;
        self.next_ticket = self.next_ticket.wrapping_add(1).max(1);
        let replaced = self.entries.insert(
            id,
            Request {
                ticket,
                handle: handle.clone(),
                state,
                callbacks: callback.into_iter().collect(),
                seen_in_frame: self.frame,
            },
        );
        debug_assert!(replaced.is_none());
        self.waiting.push_back((id, ticket));
    }

    pub(super) fn dispatch(&mut self, worker: &Worker) {
        while let Some((id, ticket)) = self.waiting.pop_front() {
            let Some(request) = self.entries.get_mut(&id) else {
                continue;
            };

            if request.ticket != ticket {
                continue;
            }

            let accepted = match &request.state {
                State::Decode { wake } => worker.try_decode(ticket, &request.handle, *wake),
                State::Upload { pixels, wake } => {
                    worker.try_upload(ticket, &request.handle, pixels, *wake)
                }
                State::InFlight => continue,
            };

            if accepted {
                request.state = State::InFlight;
            } else {
                self.waiting.push_front((id, ticket));
                break;
            }
        }
    }

    pub(super) fn remove_matching(&mut self, id: core::image::Id, ticket: u64) -> Option<Request> {
        match self.entries.entry(id) {
            std::collections::hash_map::Entry::Occupied(occupied)
                if occupied.get().ticket == ticket =>
            {
                Some(occupied.remove())
            }
            _ => None,
        }
    }

    pub(super) fn trim(&mut self) {
        let frame = self.frame;
        self.entries
            .retain(|_, request| request.seen_in_frame == frame || !request.callbacks.is_empty());
        self.waiting.retain(|(id, ticket)| {
            self.entries
                .get(id)
                .is_some_and(|request| request.ticket == *ticket)
        });
        self.frame = self.frame.wrapping_add(1).max(1);
    }
}

impl Cache {
    pub fn receive(&mut self) {
        while let Ok(completion) = self.worker.try_recv() {
            self.complete(completion);
        }
        self.raster.requests.dispatch(&self.worker);
    }

    pub(super) fn request_decode(&mut self, handle: &core::image::Handle, wake: fn(&Shell)) {
        use crate::image::raster::Memory;

        if self.raster.requests.touch(handle.id()) || self.raster.resident.contains(handle) {
            return;
        }

        if matches!(handle, core::image::Handle::Rgba { .. }) {
            self.raster.resident.insert(handle, Memory::load(handle));
        } else {
            self.queue_decode(handle, None, wake);
        }
    }

    pub(super) fn queue_decode(
        &mut self,
        handle: &core::image::Handle,
        callback: Option<Callback>,
        wake: fn(&Shell),
    ) {
        self.queue_request(handle, State::Decode { wake }, callback);
    }

    pub(super) fn queue_upload(
        &mut self,
        handle: &core::image::Handle,
        pixels: worker::Pixels,
        callback: Option<Callback>,
        wake: fn(&Shell),
    ) {
        self.queue_request(handle, State::Upload { pixels, wake }, callback);
    }

    fn queue_request(
        &mut self,
        handle: &core::image::Handle,
        state: State,
        callback: Option<Callback>,
    ) {
        self.raster.requests.enqueue(handle, state, callback);
        self.raster.requests.dispatch(&self.worker);
    }

    fn complete(&mut self, completion: worker::Completion) {
        use crate::image::raster::Memory;

        let (ticket, handle) = match &completion {
            worker::Completion::Ready { ticket, handle, .. }
            | worker::Completion::Failed { ticket, handle, .. } => (*ticket, handle),
        };
        let Some(request) = self.raster.requests.remove_matching(handle.id(), ticket) else {
            return;
        };

        match completion {
            worker::Completion::Ready {
                handle,
                entry,
                bind_group,
                ..
            } => {
                let allocation = if request.callbacks.is_empty() {
                    None
                } else {
                    #[allow(unsafe_code)]
                    let allocation = unsafe { core::image::allocate(&handle, entry.size()) };

                    for callback in request.callbacks {
                        callback(Ok(allocation.clone()));
                    }
                    Some(allocation.downgrade())
                };
                self.raster.resident.insert(
                    &handle,
                    Memory::Device {
                        entry,
                        bind_group: Some(bind_group),
                        allocation,
                    },
                );
            }
            worker::Completion::Failed { handle, error, .. } => {
                for callback in request.callbacks {
                    callback(Err(error.clone()));
                }
                self.raster.resident.insert(&handle, Memory::Error(error));
            }
        }
    }

    pub(super) fn trim_requests(&mut self) {
        self.raster.requests.trim();
    }
}
