//! How a widget talks back to the runtime while it handles an event.
use crate::clipboard;
use crate::damage;
use crate::event;
use crate::window;
use crate::{Clipboard, InputMethod, Window};

use std::sync::Arc;

/// The channel from a widget back to the runtime during one update.
///
/// Through it a widget publishes messages, captures the event, and asks for
/// side effects: a redraw, a relayout, a rebuilt widget tree, recomputed
/// semantics, clipboard access, an input method. Parents that run children
/// against a [`local`](Self::local) shell fold the child's effects back with
/// [`merge`](Self::merge).
#[derive(Debug)]
pub struct Shell<'a, Message> {
    window: &'a dyn Window,
    waker: Waker,
    messages: &'a mut Vec<Message>,
    event_status: event::Status,
    effects: Effects,
    input_method: InputMethod,
    clipboard: Clipboard,
}

/// Everything a widget can ask the runtime to do besides publishing
/// messages; combined field by field when shells merge.
#[derive(Debug)]
struct Effects {
    redraw: window::RedrawRequest,
    animating: bool,
    layout: Option<Diff>,
    widgets: bool,
    semantics: bool,
    damage: Vec<damage::Request>,
}

impl Effects {
    fn none() -> Self {
        Self {
            redraw: window::RedrawRequest::Wait,
            animating: false,
            layout: None,
            widgets: false,
            semantics: false,
            damage: Vec::new(),
        }
    }

    /// The union of both sets of effects: the soonest redraw, the strongest
    /// layout diff, and every flag or damage request either side raised.
    fn absorb(&mut self, other: &mut Self) {
        self.redraw = self.redraw.min(other.redraw);
        self.animating |= other.animating;
        self.layout = self.layout.max(other.layout);
        self.widgets |= other.widgets;
        self.semantics |= other.semantics;
        self.damage.append(&mut other.damage);
    }
}

impl<'a, Message> Shell<'a, Message> {
    /// A shell collecting messages into `messages`.
    pub fn new(window: &'a dyn Window, waker: Waker, messages: &'a mut Vec<Message>) -> Self {
        Self {
            window,
            waker,
            messages,
            event_status: event::Status::Ignored,
            effects: Effects::none(),
            input_method: InputMethod::Disabled,
            clipboard: Clipboard {
                reads: Vec::new(),
                write: None,
            },
        }
    }

    /// A fresh shell for the same window, collecting messages of another
    /// type into `messages`. Merge it back with [`merge`](Self::merge).
    pub fn local<'b, A>(&self, messages: &'b mut Vec<A>) -> Shell<'b, A>
    where
        'a: 'b,
    {
        Shell::new(self.window, self.waker.clone(), messages)
    }

    /// The window the event belongs to.
    pub fn window(&self) -> &'a dyn Window {
        self.window
    }

    /// A handle that wakes the runtime from any thread.
    pub fn waker(&self) -> &Waker {
        &self.waker
    }

    /// Whether nothing has been published yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Sends `message` to the application's update.
    pub fn publish(&mut self, message: Message) {
        self.messages.push(message);
    }

    /// Claims the event so no ancestor handles it.
    pub fn capture_event(&mut self) {
        self.event_status = event::Status::Captured;
    }

    /// Whether the event was claimed.
    #[must_use]
    pub fn event_status(&self) -> event::Status {
        self.event_status
    }

    /// Shorthand for `event_status() == Captured`.
    #[must_use]
    pub fn is_event_captured(&self) -> bool {
        self.event_status == event::Status::Captured
    }

    /// Asks for the next frame.
    pub fn request_redraw(&mut self) {
        self.effects.redraw = window::RedrawRequest::NextFrame;
    }

    /// Asks for a frame at `request`, keeping an earlier request if there
    /// is one.
    pub fn request_redraw_at(&mut self, request: impl Into<window::RedrawRequest>) {
        self.effects.redraw = self.effects.redraw.min(request.into());
    }

    /// The soonest frame anything asked for.
    #[must_use]
    pub fn redraw_request(&self) -> window::RedrawRequest {
        self.effects.redraw
    }

    /// Overwrites the redraw request, ignoring what was asked before. For
    /// containers that must undo a child's request; deliberately not a
    /// method so it is not reached for by accident.
    pub fn replace_redraw_request(shell: &mut Self, request: window::RedrawRequest) {
        shell.effects.redraw = request;
    }

    /// Declares that frames are needed continuously, not just once.
    ///
    /// Widgets never run their own render loop: a widget with a running
    /// animation calls this on every update (alongside
    /// [`request_redraw`](Self::request_redraw)), and the frame scheduler
    /// keeps producing frames until a whole update passes without it.
    pub fn request_animation(&mut self) {
        self.effects.animating = true;
    }

    /// Whether continuous frames were requested.
    #[must_use]
    pub fn is_animating(&self) -> bool {
        self.effects.animating
    }

    /// Asks for the clipboard contents of `kind`; they arrive later as a
    /// [`clipboard::Event::Read`].
    pub fn read_clipboard(&mut self, kind: clipboard::Kind) {
        self.clipboard.reads.push(kind);
    }

    /// Asks for `content` to be put on the clipboard; completion arrives as
    /// a [`clipboard::Event::Written`].
    pub fn write_clipboard(&mut self, content: clipboard::Content) {
        self.clipboard.write = Some(content);
    }

    /// Pending clipboard requests.
    pub fn clipboard_mut(&mut self) -> &mut Clipboard {
        &mut self.clipboard
    }

    /// Asks for an input method. Honored only while handling
    /// [`window::Event::RedrawRequested`].
    pub fn request_input_method<T: AsRef<str>>(&mut self, ime: &InputMethod<T>) {
        self.input_method.merge(ime);
    }

    /// The input method requested so far.
    #[must_use]
    pub fn input_method(&self) -> &InputMethod {
        &self.input_method
    }

    /// The input method requested so far, mutably.
    #[must_use]
    pub fn input_method_mut(&mut self) -> &mut InputMethod {
        &mut self.input_method
    }

    /// Whether layout must run again, and with which [`Diff`].
    #[must_use]
    pub fn is_layout_invalid(&self) -> Option<Diff> {
        self.effects.layout
    }

    /// Asks for a relayout of the existing widget tree.
    pub fn invalidate_layout(&mut self) {
        self.invalidate_layout_with(Diff::Skip);
    }

    /// Asks for a relayout, diffing the tree first if `diff` says so.
    pub fn invalidate_layout_with(&mut self, diff: Diff) {
        self.effects.layout = Some(diff);
    }

    /// Runs `relayout` if layout was invalidated, clearing the request.
    pub fn revalidate_layout(&mut self, relayout: impl FnOnce(Diff)) {
        if let Some(diff) = self.effects.layout.take() {
            relayout(diff);
        }
    }

    /// Whether the widget tree must be rebuilt.
    #[must_use]
    pub fn are_widgets_invalid(&self) -> bool {
        self.effects.widgets
    }

    /// Asks for the widget tree to be rebuilt from the view and laid out.
    pub fn invalidate_widgets(&mut self) {
        self.effects.widgets = true;
    }

    /// Whether the semantic tree must be recomputed.
    #[must_use]
    pub fn is_semantics_invalid(&self) -> bool {
        self.effects.semantics
    }

    /// Asks for the [semantic tree](crate::semantics) to be recomputed
    /// before assistive technology is next updated.
    pub fn invalidate_semantics(&mut self) {
        self.effects.semantics = true;
    }

    /// Records `request` and asks for the next frame.
    ///
    /// Requests are kept (see [`damage_requests`](Self::damage_requests))
    /// for precise damage tracking; until a consumer uses the regions,
    /// every request also schedules a full redraw.
    pub fn request_damage(&mut self, request: damage::Request) {
        self.effects.damage.push(request);
        self.request_redraw();
    }

    /// Every damage request recorded so far.
    pub fn damage_requests(&self) -> &[damage::Request] {
        &self.effects.damage
    }

    /// Folds a child shell back in, converting its messages with `f`.
    pub fn merge<B>(&mut self, other: Shell<'_, B>, mut f: impl FnMut(B) -> Message) {
        self.merge_map(other, |message| Some(f(message)));
    }

    /// Like [`merge`](Self::merge), but `f` may drop a message. Every other
    /// effect of the child is kept.
    pub fn merge_map<B>(
        &mut self,
        mut other: Shell<'_, B>,
        mut f: impl FnMut(B) -> Option<Message>,
    ) {
        self.messages
            .extend(other.messages.drain(..).filter_map(&mut f));

        self.event_status = self.event_status.merge(other.event_status);
        self.effects.absorb(&mut other.effects);
        self.input_method.merge(&other.input_method);
        self.clipboard.merge(&mut other.clipboard);
    }
}

/// Wakes the runtime from outside an update, for example from a worker
/// thread or a reactive signal.
#[derive(Clone)]
pub struct Waker {
    wake: Arc<dyn Fn(Option<u64>) + Send + Sync + 'static>,
}

impl Waker {
    /// A waker that calls `wake`.
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self::new_tagged(move |_| wake())
    }

    /// A waker that also receives the reactive source that caused the wake,
    /// or `None` for a plain wake.
    pub fn new_tagged(wake: impl Fn(Option<u64>) + Send + Sync + 'static) -> Self {
        Self {
            wake: Arc::new(wake),
        }
    }

    /// A waker that does nothing.
    pub fn noop() -> Self {
        Self::new_tagged(|_| {})
    }

    /// Wakes the runtime.
    pub fn wake(&self) {
        (self.wake)(None);
    }

    /// Wakes the runtime on behalf of the reactive source `source`.
    pub fn wake_reactive(&self, source: u64) {
        (self.wake)(Some(source));
    }
}

impl std::fmt::Debug for Waker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Waker")
    }
}

/// Whether a relayout must diff the widget tree first. `Perform` is the
/// stronger request and wins when two are merged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Diff {
    /// Lay out the existing tree.
    Skip,
    /// Diff the tree against the view, then lay out.
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

    #[test]
    fn merge_keeps_the_strongest_layout_request() {
        for (mine, theirs, expected) in [
            (None, None, None),
            (Some(Diff::Skip), None, Some(Diff::Skip)),
            (None, Some(Diff::Perform), Some(Diff::Perform)),
            (Some(Diff::Perform), Some(Diff::Skip), Some(Diff::Perform)),
        ] {
            let mut messages_a = Vec::new();
            let mut main_shell = shell(&mut messages_a);
            let mut messages_b = Vec::new();
            let mut other = shell(&mut messages_b);

            if let Some(diff) = mine {
                main_shell.invalidate_layout_with(diff);
            }
            if let Some(diff) = theirs {
                other.invalidate_layout_with(diff);
            }

            main_shell.merge(other, |message| message);
            assert_eq!(main_shell.is_layout_invalid(), expected);
        }
    }
}
