//! Communicate with the taim runtime from widgets.
use crate::clipboard;
use crate::damage;
use crate::event;
use crate::window;
use crate::{Clipboard, InputMethod, Window};

use std::sync::Arc;

/// A connection to the state of a shell.
///
/// A [`Widget`] can leverage a [`Shell`] to trigger changes in an application,
/// like publishing messages or invalidating the current layout.
///
/// [`Widget`]: crate::Widget
#[derive(Debug)]
pub struct Shell<'a, Message> {
    window: &'a dyn Window,
    messages: &'a mut Vec<Message>,
    waker: Waker,
    event_status: event::Status,
    redraw_request: window::RedrawRequest,
    input_method: InputMethod,
    is_layout_invalid: Option<Diff>,
    are_widgets_invalid: bool,
    is_semantics_invalid: bool,
    damage_requests: Vec<damage::Request>,
    is_animating: bool,
    clipboard: Clipboard,
}

impl<'a, Message> Shell<'a, Message> {
    /// Creates a new [`Shell`] with the provided buffer of messages.
    pub fn new(window: &'a dyn Window, waker: Waker, messages: &'a mut Vec<Message>) -> Self {
        Self {
            window,
            messages,
            waker,
            event_status: event::Status::Ignored,
            redraw_request: window::RedrawRequest::Wait,
            is_layout_invalid: None,
            are_widgets_invalid: false,
            is_semantics_invalid: false,
            damage_requests: Vec::new(),
            is_animating: false,
            input_method: InputMethod::Disabled,
            clipboard: Clipboard {
                reads: Vec::new(),
                write: None,
            },
        }
    }

    /// Creates a new [`Shell`] from the current one with the given list of local messages.
    pub fn local<'b, A>(&self, messages: &'b mut Vec<A>) -> Shell<'b, A>
    where
        'a: 'b,
    {
        Shell::new(self.window, self.waker.clone(), messages)
    }

    /// Returns the [`Window`] of the [`Shell`].
    pub fn window(&self) -> &'a dyn Window {
        self.window
    }

    /// Returns the [`Waker`] of the [`Shell`].
    pub fn waker(&self) -> &Waker {
        &self.waker
    }

    /// Returns true if the [`Shell`] contains no published messages
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Publish the given `Message` for an application to process it.
    pub fn publish(&mut self, message: Message) {
        self.messages.push(message);
    }

    /// Marks the current event as captured. Prevents "event bubbling".
    ///
    /// A widget should capture an event when no ancestor should
    /// handle it.
    pub fn capture_event(&mut self) {
        self.event_status = event::Status::Captured;
    }

    /// Returns the current [`event::Status`] of the [`Shell`].
    #[must_use]
    pub fn event_status(&self) -> event::Status {
        self.event_status
    }

    /// Returns whether the current event has been captured.
    #[must_use]
    pub fn is_event_captured(&self) -> bool {
        self.event_status == event::Status::Captured
    }

    /// Requests a new frame to be drawn as soon as possible.
    pub fn request_redraw(&mut self) {
        self.redraw_request = window::RedrawRequest::NextFrame;
    }

    /// Requests a new frame to be drawn at the given [`window::RedrawRequest`].
    pub fn request_redraw_at(&mut self, redraw_request: impl Into<window::RedrawRequest>) {
        self.redraw_request = self.redraw_request.min(redraw_request.into());
    }

    /// Returns the request a redraw should happen, if any.
    #[must_use]
    pub fn redraw_request(&self) -> window::RedrawRequest {
        self.redraw_request
    }

    /// Replaces the redraw request of the [`Shell`]; without conflict resolution.
    ///
    /// This is useful if you want to overwrite the redraw request to a previous value.
    /// Since it's a fairly advanced use case and should rarely be used, it is a static
    /// method.
    pub fn replace_redraw_request(shell: &mut Self, redraw_request: window::RedrawRequest) {
        shell.redraw_request = redraw_request;
    }

    /// Declares that continuous redraws are needed (e.g. an in-progress
    /// animation), rather than a single one-shot redraw.
    ///
    /// This is the "widgets never create independent render loops" rule
    /// (`phase.md` Phase 6) made explicit: a widget with an active
    /// animation calls this every time it updates (alongside
    /// [`Shell::request_redraw`], which it should still call to get the
    /// next frame scheduled) so a frame scheduler
    /// (`runtime::frame_scheduler::Scheduler`) can tell "one more frame
    /// is needed" apart from "frames are needed continuously until this
    /// stops being called," instead of inferring a loop from the mere
    /// repetition of one-shot requests.
    pub fn request_animation(&mut self) {
        self.is_animating = true;
    }

    /// Returns whether continuous redraws have been requested on this
    /// [`Shell`] (see [`Shell::request_animation`]).
    #[must_use]
    pub fn is_animating(&self) -> bool {
        self.is_animating
    }

    /// Requests the runtime to read the clipboard contents expecting the given [`clipboard::Kind`].
    ///
    /// The runtime will produce a [`clipboard::Event::Read`] when the contents have been read.
    pub fn read_clipboard(&mut self, kind: clipboard::Kind) {
        self.clipboard.reads.push(kind);
    }

    /// Requests the runtime to write the given [`clipboard::Content`] to the clipboard.
    ///
    /// The runtime will produce a [`clipboard::Event::Written`] when the contents have been written.
    pub fn write_clipboard(&mut self, content: clipboard::Content) {
        self.clipboard.write = Some(content);
    }

    /// Returns the [`Clipboard`] requests of the [`Shell`], mutably.
    pub fn clipboard_mut(&mut self) -> &mut Clipboard {
        &mut self.clipboard
    }

    /// Requests the current [`InputMethod`] strategy.
    ///
    /// __Important__: This request will only be honored by the
    /// [`Shell`] only during a [`window::Event::RedrawRequested`].
    pub fn request_input_method<T: AsRef<str>>(&mut self, ime: &InputMethod<T>) {
        self.input_method.merge(ime);
    }

    /// Returns the current [`InputMethod`] strategy.
    #[must_use]
    pub fn input_method(&self) -> &InputMethod {
        &self.input_method
    }

    /// Returns the current [`InputMethod`] strategy.
    #[must_use]
    pub fn input_method_mut(&mut self) -> &mut InputMethod {
        &mut self.input_method
    }

    /// Returns whether the current layout is invalid or not.
    #[must_use]
    pub fn is_layout_invalid(&self) -> Option<Diff> {
        self.is_layout_invalid
    }

    /// Invalidates the current application layout.
    ///
    /// The shell will relayout the application widgets.
    pub fn invalidate_layout(&mut self) {
        self.invalidate_layout_with(Diff::Skip);
    }

    /// Invalidates the current application layout with the following [`Diff`] strategy.
    pub fn invalidate_layout_with(&mut self, diff: Diff) {
        self.is_layout_invalid = Some(diff);
    }

    /// Triggers the given function if the layout is invalid, cleaning it in the
    /// process.
    pub fn revalidate_layout(&mut self, f: impl FnOnce(Diff)) {
        if let Some(diff) = self.is_layout_invalid.take() {
            f(diff);
        }
    }

    /// Returns whether the widgets of the current application have been
    /// invalidated.
    #[must_use]
    pub fn are_widgets_invalid(&self) -> bool {
        self.are_widgets_invalid
    }

    /// Invalidates the current application widgets.
    ///
    /// The shell will rebuild and relayout the widget tree.
    pub fn invalidate_widgets(&mut self) {
        self.are_widgets_invalid = true;
    }

    /// Returns whether the semantic tree of the current application has
    /// been invalidated.
    #[must_use]
    pub fn is_semantics_invalid(&self) -> bool {
        self.is_semantics_invalid
    }

    /// Invalidates the current application's semantic tree (see
    /// [`crate::semantics`]).
    ///
    /// The shell will recompute semantics before the next presentation to
    /// assistive technology. There is no consumer of this yet -- Phase
    /// 13 is the first phase that reads [`Shell::is_semantics_invalid`];
    /// until then, calling this only records the request.
    pub fn invalidate_semantics(&mut self) {
        self.is_semantics_invalid = true;
    }

    /// Requests that the given [`damage::Request`] be applied.
    ///
    /// Until Phase 4 lands real damage-region tracking, every
    /// [`damage::Request`] conservatively invalidates the entire redraw
    /// path (equivalent to calling [`Shell::request_redraw`]) rather
    /// than attempting to track precise regions -- the request is still
    /// recorded (see [`Shell::damage_requests`]) so a later phase can
    /// switch this method's body over to precise tracking without
    /// changing any widget's call site.
    pub fn request_damage(&mut self, request: damage::Request) {
        self.damage_requests.push(request);
        self.request_redraw();
    }

    /// Returns every [`damage::Request`] accumulated on this [`Shell`]
    /// so far.
    ///
    /// No runtime consumes this yet; it exists so damage requests are
    /// not silently dropped once widgets start producing them, ahead of
    /// Phase 4 wiring a real consumer.
    pub fn damage_requests(&self) -> &[damage::Request] {
        &self.damage_requests
    }

    /// Merges the current [`Shell`] with another one by applying the given
    /// function to the messages of the latter.
    ///
    /// This method is useful for composition.
    pub fn merge<B>(&mut self, other: Shell<'_, B>, mut f: impl FnMut(B) -> Message) {
        self.merge_map(other, |message| Some(f(message)));
    }

    /// Merges another shell while allowing local messages to be filtered.
    ///
    /// This is useful for encapsulated components whose local update can
    /// intentionally consume an event without publishing a parent message.
    /// All non-message effects are still preserved.
    pub fn merge_map<B>(
        &mut self,
        mut other: Shell<'_, B>,
        mut f: impl FnMut(B) -> Option<Message>,
    ) {
        self.messages
            .extend(other.messages.drain(..).filter_map(&mut f));

        self.is_layout_invalid = match (self.is_layout_invalid, other.is_layout_invalid) {
            (Some(a), Some(b)) => Some(a.max(b)),
            _ => self.is_layout_invalid.or(other.is_layout_invalid),
        };

        self.are_widgets_invalid = self.are_widgets_invalid || other.are_widgets_invalid;
        self.is_semantics_invalid = self.is_semantics_invalid || other.is_semantics_invalid;
        self.is_animating = self.is_animating || other.is_animating;
        self.damage_requests.append(&mut other.damage_requests);
        self.redraw_request = self.redraw_request.min(other.redraw_request);
        self.event_status = self.event_status.merge(other.event_status);

        self.input_method.merge(&other.input_method);
        self.clipboard.merge(&mut other.clipboard);
    }
}

/// A waker can be used to wake up the taim runtime and, consequently, trigger
/// wake events concurrently from widget logic.
#[derive(Clone)]
pub struct Waker {
    wake: Arc<dyn Fn(Option<u64>) + Send + Sync + 'static>,
}

impl Waker {
    /// Creates a new [`Waker`] with the given `wake` function.
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            wake: Arc::new(move |_| wake()),
        }
    }

    /// Creates a waker that preserves an optional reactive routing token.
    pub fn new_tagged(wake: impl Fn(Option<u64>) + Send + Sync + 'static) -> Self {
        Self {
            wake: Arc::new(wake),
        }
    }

    /// Creates a new [`Waker`] that does nothing.
    pub fn noop() -> Self {
        Self::new(|| {})
    }

    /// Wakes up the taim runtime as soon as possible.
    ///
    /// You normally want to call this concurrently (e.g. from a different thread).
    pub fn wake(&self) {
        (self.wake)(None);
    }

    /// Wakes the runtime for one process-local reactive source.
    pub fn wake_reactive(&self, source: u64) {
        (self.wake)(Some(source));
    }
}

impl std::fmt::Debug for Waker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Waker").finish()
    }
}

/// The diffing strategy to follow when invalidating some layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Diff {
    /// Skips the diffing step.
    Skip,
    /// Performs diffing again before layouting.
    Perform,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::damage::{Cause, Region, Request};
    use crate::window;

    fn shell(messages: &mut Vec<()>) -> Shell<'_, ()> {
        Shell::new(&window::Headless, Waker::noop(), messages)
    }

    #[test]
    fn tagged_waker_preserves_reactive_tokens_and_general_wakes() {
        let observed = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = Arc::clone(&observed);
        let waker = Waker::new_tagged(move |source| {
            sink.lock().expect("wake log lock poisoned").push(source);
        });

        waker.wake_reactive(42);
        waker.wake();

        assert_eq!(
            *observed.lock().expect("wake log lock poisoned"),
            vec![Some(42), None]
        );
    }

    #[test]
    fn a_fresh_shell_has_no_invalid_semantics_or_damage() {
        let mut messages = Vec::new();
        let shell = shell(&mut messages);

        assert!(!shell.is_semantics_invalid());
        assert!(shell.damage_requests().is_empty());
    }

    #[test]
    fn invalidate_semantics_sets_the_flag() {
        let mut messages = Vec::new();
        let mut shell = shell(&mut messages);

        shell.invalidate_semantics();

        assert!(shell.is_semantics_invalid());
    }

    #[test]
    fn request_damage_records_the_request_and_requests_a_redraw() {
        let mut messages = Vec::new();
        let mut shell = shell(&mut messages);

        let request = Request::full(Cause::Content);
        shell.request_damage(request);

        assert_eq!(shell.damage_requests(), &[request]);
        assert_eq!(shell.redraw_request(), window::RedrawRequest::NextFrame);
    }

    #[test]
    fn merge_combines_semantics_invalidation_and_damage_requests() {
        let mut messages_a = Vec::new();
        let mut main_shell = shell(&mut messages_a);

        let mut messages_b = Vec::new();
        let mut other = shell(&mut messages_b);
        other.invalidate_semantics();
        other.request_damage(Request::full(Cause::Layout));

        main_shell.merge(other, |message| message);

        assert!(main_shell.is_semantics_invalid());
        assert_eq!(main_shell.damage_requests().len(), 1);
        assert_eq!(main_shell.damage_requests()[0].cause, Cause::Layout);
    }

    #[test]
    fn request_damage_accepts_an_area_region() {
        let mut messages = Vec::new();
        let mut shell = shell(&mut messages);

        let bounds = crate::Rectangle::new(crate::Point::ORIGIN, crate::Size::new(10.0, 10.0));
        let region = Region::area(bounds).expect("well-formed bounds");

        shell.request_damage(Request::new(region, Cause::Interaction));

        assert_eq!(shell.damage_requests()[0].region, region);
    }

    #[test]
    fn a_fresh_shell_is_not_animating() {
        let mut messages = Vec::new();
        let shell = shell(&mut messages);

        assert!(!shell.is_animating());
    }

    #[test]
    fn request_animation_sets_the_flag() {
        let mut messages = Vec::new();
        let mut shell = shell(&mut messages);

        shell.request_animation();

        assert!(shell.is_animating());
    }

    #[test]
    fn merge_combines_animation_requests() {
        let mut messages_a = Vec::new();
        let mut main_shell = shell(&mut messages_a);

        let mut messages_b = Vec::new();
        let mut other = shell(&mut messages_b);
        other.request_animation();

        main_shell.merge(other, |message| message);

        assert!(main_shell.is_animating());
    }

    #[test]
    fn merge_without_either_side_animating_stays_not_animating() {
        let mut messages_a = Vec::new();
        let mut main_shell = shell(&mut messages_a);

        let mut messages_b = Vec::new();
        let other = shell(&mut messages_b);

        main_shell.merge(other, |message| message);

        assert!(!main_shell.is_animating());
    }
}
