//! A signal-backed widget subtree.

use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::{self, Widget};
use crate::core::{self, Element, Event, Length, Rectangle, Shell, Size, Vector};
use crate::lazy::Lazy;

use taim_signal::{Readable, Revision, SourceId, Subscription};

/// The strongest invalidation a reactive view is allowed to produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invalidation {
    /// The widget tree and geometry stay fixed; paint reads the live source.
    Value,
    /// The view may change measurement, paint, and semantics.
    Layout,
    /// The view keeps the same widget type and geometry; only paint changes.
    Paint,
}

/// A widget subtree whose construction is driven by a reactive source.
///
/// Unlike a plain lazy widget, this wrapper keeps the source itself. Once it
/// has observed a runtime event, source changes wake the event loop; the next
/// event synchronizes and lays out only this subtree without requiring an
/// application message or a new root `view`.
pub struct Reactive<'a, Message, Theme, Renderer, Source, View>
where
    Source: Readable,
{
    source: Source,
    lazy: Lazy<'a, Message, Theme, Renderer, (SourceId, Revision), View>,
    subscription: Option<Subscription>,
    invalidation: Invalidation,
}

impl<'a, Message, Theme, Renderer, Source, View>
    Reactive<'a, Message, Theme, Renderer, Source, View>
where
    Source: Readable + 'a,
    View: Into<Element<'static, Message, Theme, Renderer>>,
{
    /// Creates a reactive subtree.
    pub fn new(source: Source, view: impl Fn(Source::Value) -> View + 'a) -> Self {
        let dependency = (source.source_id(), source.revision());
        let view_source = source.clone();

        Self {
            source,
            lazy: Lazy::new(dependency, move |_| view(view_source.get())),
            subscription: None,
            invalidation: Invalidation::Layout,
        }
    }

    /// Creates a fixed widget subtree that reads `source` while painting.
    ///
    /// The view is built once and receives a clone of the live source instead
    /// of a snapshot of its value. A source change invalidates the retained
    /// paint range, but does not reconstruct, diff, lay out, or recompute the
    /// semantics of the subtree. The returned widget must therefore keep its
    /// structure and geometry independent of the source value.
    pub fn value(source: Source, view: impl Fn(Source) -> View + 'a) -> Self {
        let dependency = (source.source_id(), Revision::INITIAL);
        let view_source = source.clone();

        Self {
            source,
            lazy: Lazy::new(dependency, move |_| view(view_source.clone())),
            subscription: None,
            invalidation: Invalidation::Value,
        }
    }

    /// Promises that source changes preserve this subtree's widget type,
    /// geometry, and semantics, allowing layout to remain cached.
    #[must_use]
    pub fn paint_only(mut self) -> Self {
        self.invalidation = Invalidation::Paint;
        self
    }

    fn synchronize(&mut self, tree: &mut Tree) -> bool
    where
        Message: 'static,
        Theme: 'static,
        Renderer: core::Renderer + 'static,
        View: 'static,
    {
        if self.invalidation == Invalidation::Value {
            return false;
        }

        let dependency = self.dependency();
        let revision_before = tree.revision();
        self.lazy.replace_dependency(dependency);
        Widget::diff(&mut self.lazy, tree);
        let changed = tree.revision() != revision_before;

        if changed && self.invalidation == Invalidation::Paint {
            tree.clear_dirty(tree::DirtyFlags::LAYOUT | tree::DirtyFlags::SEMANTICS);
            for child in &mut tree.children {
                child.clear_dirty(tree::DirtyFlags::LAYOUT | tree::DirtyFlags::SEMANTICS);
            }
        }

        changed
    }

    fn dependency(&self) -> (SourceId, Revision) {
        (
            self.source.source_id(),
            if self.invalidation == Invalidation::Value {
                Revision::INITIAL
            } else {
                self.source.revision()
            },
        )
    }

    fn ensure_wake_subscription(&mut self, shell: &Shell<'_, Message>) {
        if self.subscription.is_some() {
            return;
        }

        let waker = shell.waker().clone();
        let source = self.source.source_id().routing_token();
        self.subscription = Some(self.source.subscribe(move |_| waker.wake_reactive(source)));
    }
}

impl<'a, Message, Theme, Renderer, Source, View> Widget<Message, Theme, Renderer>
    for Reactive<'a, Message, Theme, Renderer, Source, View>
where
    Source: Readable + 'a,
    View: Into<Element<'static, Message, Theme, Renderer>> + 'static,
    Message: 'static,
    Theme: 'static,
    Renderer: core::Renderer + 'static,
{
    fn tag(&self) -> tree::Tag {
        Widget::tag(&self.lazy)
    }

    fn state(&self) -> tree::State {
        Widget::state(&self.lazy)
    }

    fn diff(&mut self, tree: &mut Tree) {
        self.lazy.replace_dependency(self.dependency());
        Widget::diff(&mut self.lazy, tree);
    }

    fn size(&self) -> Size<Length> {
        Widget::size(&self.lazy)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let _ = self.synchronize(tree);
        Widget::layout(&mut self.lazy, tree, renderer, limits)
    }

    fn visit_children(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn widget::visit::Visit),
    ) {
        Widget::visit_children(&mut self.lazy, tree, layout, visit);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        Widget::operate(&mut self.lazy, tree, layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.ensure_wake_subscription(shell);

        let targeted = if let Event::Reactive(source) = event {
            if *source != self.source.source_id().routing_token() {
                return;
            }

            true
        } else {
            false
        };

        if self.invalidation == Invalidation::Value && (targeted || matches!(event, Event::Waken)) {
            tree.mark_dirty(tree::DirtyFlags::PAINT);

            if let Some(child) = tree.children.first_mut() {
                child.mark_dirty(tree::DirtyFlags::PAINT);
            }

            shell.request_redraw();
        } else if self.synchronize(tree) {
            if self.invalidation == Invalidation::Layout {
                shell.invalidate_layout();
                shell.invalidate_semantics();
            }
            shell.request_redraw();
        }

        // `Reactive` is an internal routing event, not user input. The
        // matching wrapper has already consumed it by synchronizing its lazy
        // subtree above, so forwarding it through the freshly-built child
        // would only walk the same branch a second time. Untagged wake events
        // remain forwarded for compatibility with widgets that use the
        // general-purpose runtime waker.
        if targeted {
            return;
        }

        Widget::update(
            &mut self.lazy,
            tree,
            event,
            layout,
            cursor,
            renderer,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        Widget::mouse_interaction(&self.lazy, tree, layout, cursor, viewport, renderer)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        Widget::draw(
            &self.lazy, tree, renderer, theme, style, layout, cursor, viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        Widget::overlay(
            &mut self.lazy,
            tree,
            layout,
            renderer,
            viewport,
            translation,
        )
    }

    fn is_void(&self) -> bool {
        Widget::is_void(&self.lazy)
    }

    fn cache_policy(&self) -> Option<core::cache::Policy> {
        Widget::cache_policy(&self.lazy)
    }

    fn reactive_source(&self) -> Option<u64> {
        Some(self.source.source_id().routing_token())
    }
}

impl<'a, Message, Theme, Renderer, Source, View>
    From<Reactive<'a, Message, Theme, Renderer, Source, View>>
    for Element<'a, Message, Theme, Renderer>
where
    Source: Readable + 'a,
    View: Into<Element<'static, Message, Theme, Renderer>> + 'static,
    Message: 'static,
    Theme: 'static,
    Renderer: core::Renderer + 'static,
{
    fn from(reactive: Reactive<'a, Message, Theme, Renderer, Source, View>) -> Self {
        Self::new(reactive)
    }
}
