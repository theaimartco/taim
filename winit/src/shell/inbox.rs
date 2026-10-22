//! Per-window input queue between two engine turns.
use crate::core::{self, mouse, touch, window};

use rustc_hash::FxHashMap;

/// Events waiting to be routed, grouped by the window they belong to.
///
/// Order inside a window is preserved exactly; order across windows does not
/// matter because every interface update is window-scoped. Buffers are kept
/// between turns so a burst of input (a resize drag, a fling) does not
/// reallocate the same vectors every frame.
#[derive(Default)]
pub(super) struct Inbox {
    windows: FxHashMap<window::Id, Vec<core::Event>>,
    queued: usize,
}

impl Inbox {
    /// Queues `event` for `window`, folding it into the previous event when
    /// the previous one is an absolute sample it fully supersedes.
    #[inline]
    pub(super) fn push(&mut self, window: window::Id, event: core::Event) {
        let queue = self.windows.entry(window).or_default();

        if let Some(last) = queue.last_mut()
            && supersedes(last, &event)
        {
            *last = event;
            return;
        }

        queue.push(event);
        self.queued = self.queued.saturating_add(1);
    }

    #[inline]
    pub(super) fn is_empty(&self) -> bool {
        self.queued == 0
    }

    /// The events still queued for `window`, if there are any.
    #[inline]
    pub(super) fn pending_for(&mut self, window: window::Id) -> Option<&mut Vec<core::Event>> {
        self.windows
            .get_mut(&window)
            .filter(|events| !events.is_empty())
    }

    /// Empties every queue, handing each leftover event to `sink`.
    pub(super) fn drain_all(&mut self, mut sink: impl FnMut(window::Id, core::Event)) {
        for (window, events) in &mut self.windows {
            for event in events.drain(..) {
                sink(*window, event);
            }
        }
    }

    /// Marks the batch consumed while keeping the allocations.
    #[inline]
    pub(super) fn finish_batch(&mut self) {
        self.queued = 0;
    }

    /// Forgets queues of windows that no longer exist.
    pub(super) fn retain_windows(&mut self, mut is_open: impl FnMut(window::Id) -> bool) {
        self.windows.retain(|window, _| is_open(*window));

        let floor = self.windows.len().max(8);
        if self.windows.capacity() > floor.saturating_mul(4) {
            self.windows.shrink_to(floor);
        }
    }
}

/// Whether `next` makes `previous` redundant.
///
/// Only adjacent absolute samples of the same source fold: the newest cursor
/// position, the newest position of the same finger, a window geometry event
/// its successor replaces. Button transitions, wheel deltas, file drops and
/// focus changes are boundaries that gestures and observers rely on.
#[inline]
fn supersedes(previous: &core::Event, next: &core::Event) -> bool {
    use core::Event::{Mouse, Touch, Window};

    match (previous, next) {
        (Window(before), Window(after)) => after.supersedes_adjacent(before),
        (Mouse(mouse::Event::CursorMoved { .. }), Mouse(mouse::Event::CursorMoved { .. })) => true,
        (
            Touch(touch::Event::FingerMoved { id: before, .. }),
            Touch(touch::Event::FingerMoved { id: after, .. }),
        ) => before == after,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Point, Size};

    fn cursor_at(x: f32) -> core::Event {
        core::Event::Mouse(mouse::Event::CursorMoved {
            position: Point::new(x, 0.0),
        })
    }

    fn finger_at(finger: u64, x: f32) -> core::Event {
        core::Event::Touch(touch::Event::FingerMoved {
            id: touch::Finger(finger),
            position: Point::new(x, 2.0),
            metadata: core::pointer::Metadata::NONE,
        })
    }

    fn queued(inbox: &mut Inbox, window: window::Id) -> Vec<core::Event> {
        inbox.pending_for(window).cloned().unwrap_or_default()
    }

    #[test]
    fn cursor_samples_fold_into_the_newest() {
        let window = window::Id::unique();
        let mut inbox = Inbox::default();

        for x in [1.0, 2.0, 3.0] {
            inbox.push(window, cursor_at(x));
        }

        assert_eq!(inbox.queued, 1);
        assert_eq!(queued(&mut inbox, window), vec![cursor_at(3.0)]);
    }

    #[test]
    fn button_transitions_split_cursor_runs() {
        let window = window::Id::unique();
        let mut inbox = Inbox::default();
        let left = mouse::Button::Left;

        inbox.push(window, cursor_at(1.0));
        inbox.push(
            window,
            core::Event::Mouse(mouse::Event::ButtonPressed(left)),
        );
        inbox.push(window, cursor_at(2.0));
        inbox.push(
            window,
            core::Event::Mouse(mouse::Event::ButtonReleased(left)),
        );
        inbox.push(window, cursor_at(3.0));

        assert_eq!(inbox.queued, 5);
        assert_eq!(queued(&mut inbox, window).len(), 5);
    }

    #[test]
    fn fingers_fold_only_with_themselves() {
        let window = window::Id::unique();
        let mut inbox = Inbox::default();

        inbox.push(window, finger_at(1, 1.0));
        inbox.push(window, finger_at(2, 2.0));
        inbox.push(window, finger_at(1, 3.0));
        inbox.push(window, finger_at(1, 4.0));

        assert_eq!(
            queued(&mut inbox, window),
            vec![finger_at(1, 1.0), finger_at(2, 2.0), finger_at(1, 4.0)]
        );
    }

    #[test]
    fn resizes_fold_but_close_requests_never_do() {
        let window = window::Id::unique();
        let mut inbox = Inbox::default();
        let resized = |w, h| core::Event::Window(window::Event::Resized(Size::new(w, h)));

        inbox.push(window, resized(10.0, 10.0));
        inbox.push(window, resized(20.0, 30.0));
        assert_eq!(inbox.queued, 1);

        inbox.push(window, core::Event::Window(window::Event::CloseRequested));
        inbox.push(window, core::Event::Window(window::Event::CloseRequested));
        assert_eq!(inbox.queued, 3);
        assert_eq!(queued(&mut inbox, window)[0], resized(20.0, 30.0));
    }

    #[test]
    fn windows_keep_independent_queues_until_drained() {
        let a = window::Id::unique();
        let b = window::Id::unique();
        let mut inbox = Inbox::default();

        inbox.push(a, cursor_at(1.0));
        inbox.push(b, cursor_at(2.0));
        inbox.push(a, cursor_at(3.0));

        assert_eq!(queued(&mut inbox, a), vec![cursor_at(3.0)]);
        assert_eq!(queued(&mut inbox, b), vec![cursor_at(2.0)]);
        assert_eq!(inbox.queued, 2);

        let mut drained = 0;
        inbox.drain_all(|_, _| drained += 1);
        inbox.finish_batch();

        assert_eq!(drained, 2);
        assert!(inbox.is_empty());
        assert!(inbox.pending_for(a).is_none());
    }
}
