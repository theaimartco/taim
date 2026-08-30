//! A page-stack widget with animated route transitions.
//!
//! [`Navigator`] manages a stack of page-building closures and
//! orchestrates enter/exit transitions between them. It supports:
//!
//! - **Push** with configurable per-transition animation
//! - **Animated** transitions — entering page slides/fades in while the
//!   exiting page slides/fades out
//! - **Interruptible** — pushing mid-animation seamlessly starts a new
//!   transition from the current visual state

use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::renderer;
use crate::core::time::Instant;
use crate::core::widget::tree::{self, Tree};
use crate::core::window;
use crate::core::{self, Element, Event, Length, Rectangle, Shell, Size, Widget};

// Navigation and the rest of Taim use the same transition contract. Keeping
// this re-export preserves the widget-facing API without maintaining a
// second, incompatible implementation here.
pub use taim_motion::{PageTransition, TransformState};

// -----------------------------------------------------------------------
// Animation state
// -----------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct TransitionAnim {
    transition: PageTransition,
    kind: TransitionKind,
    started_at: Instant,
    duration: f64,
    settled: bool,
}

impl TransitionAnim {
    fn new(transition: PageTransition, kind: TransitionKind, now: Instant) -> Self {
        let duration = transition.duration_seconds().unwrap_or(f64::INFINITY);
        Self {
            transition,
            kind,
            started_at: now,
            duration,
            settled: duration == 0.0,
        }
    }

    fn progress(&self, now: Instant) -> f32 {
        if self.settled {
            return 1.0;
        }
        let elapsed = now.duration_since(self.started_at).as_secs_f64();
        if let PageTransition::SpringSlide { spring } = self.transition {
            let mut motion = taim_motion::Spring::with_config(0.0, spring);
            motion.set_target(1.0);
            motion.advance(elapsed.min(10.0) as f32).clamp(0.0, 1.0)
        } else if self.duration > 0.0 {
            (elapsed / self.duration).min(1.0) as f32
        } else {
            1.0
        }
    }

    fn advance(&mut self, now: Instant) -> bool {
        if self.settled {
            return false;
        }
        let elapsed = now.duration_since(self.started_at).as_secs_f64();
        self.settled = if let PageTransition::SpringSlide { spring } = self.transition {
            let mut motion = taim_motion::Spring::with_config(0.0, spring);
            motion.set_target(1.0);
            let _ = motion.advance(elapsed.min(10.0) as f32);

            motion.is_at_rest() || elapsed >= 10.0
        } else {
            self.progress(now) >= 1.0
        };

        !self.settled
    }

    fn enter_state(&self, now: Instant) -> TransformState {
        self.transition.enter_at(self.progress(now))
    }

    fn exit_state(&self, now: Instant) -> TransformState {
        self.transition.exit_at(self.progress(now))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransitionKind {
    Push,
    Pop,
    Replace,
}

// -----------------------------------------------------------------------
// Navigator widget
// -----------------------------------------------------------------------

struct NavigatorState {
    active_transition: Option<TransitionAnim>,
    last_instant: Instant,
}

impl Default for NavigatorState {
    fn default() -> Self {
        Self {
            active_transition: None,
            last_instant: Instant::now(),
        }
    }
}

/// A page entry in the navigation stack.
struct Page<'a, Message, Theme, Renderer> {
    element: Element<'a, Message, Theme, Renderer>,
}

/// A widget that manages a stack of pages with animated transitions.
///
/// # Example
///
/// ```ignore
/// use taim_widget::navigator::{Navigator, PageTransition};
///
/// Navigator::new(page_a)
///     .push(page_b, PageTransition::FORWARD)
/// ```
pub struct Navigator<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: core::Renderer,
{
    pages: Vec<Page<'a, Message, Theme, Renderer>>,
    departing: Option<Page<'a, Message, Theme, Renderer>>,
    pending_transition: Option<(PageTransition, TransitionKind)>,
    width: Length,
    height: Length,
}

impl<'a, Message, Theme, Renderer> Navigator<'a, Message, Theme, Renderer>
where
    Renderer: core::Renderer,
{
    /// Creates a new [`Navigator`] with a single root page.
    pub fn new(root: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            pages: vec![Page {
                element: root.into(),
            }],
            departing: None,
            pending_transition: None,
            width: Length::Fill,
            height: Length::Fill,
        }
    }

    /// Pushes a page onto the stack with the given transition.
    pub fn push(
        mut self,
        page: impl Into<Element<'a, Message, Theme, Renderer>>,
        transition: PageTransition,
    ) -> Self {
        self.pages.push(Page {
            element: page.into(),
        });
        self.pending_transition = Some((transition, TransitionKind::Push));
        self
    }

    /// Pops the top page, retaining the root page, and animates the revealed
    /// page with the supplied transition.
    pub fn pop(mut self, transition: PageTransition) -> Self {
        if self.pages.len() > 1 {
            self.departing = self.pages.pop();
            self.pending_transition = Some((transition, TransitionKind::Pop));
        }
        self
    }

    /// Replaces the top page without changing the stack depth.
    pub fn replace(
        mut self,
        page: impl Into<Element<'a, Message, Theme, Renderer>>,
        transition: PageTransition,
    ) -> Self {
        let page = page.into();
        if let Some(top) = self.pages.last_mut() {
            let outgoing = std::mem::replace(&mut top.element, page);
            self.departing = Some(Page { element: outgoing });
        } else {
            self.pages.push(Page { element: page });
        }
        self.pending_transition = Some((transition, TransitionKind::Replace));
        self
    }

    /// Sets the width of the [`Navigator`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`Navigator`].
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Navigator<'_, Message, Theme, Renderer>
where
    Renderer: core::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<NavigatorState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(NavigatorState::default())
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn diff(&mut self, tree: &mut Tree) {
        let visual_len = self.pages.len() + usize::from(self.departing.is_some());

        // `replace` needs two visual children, but the existing top child's
        // state belongs to the outgoing page. Move that state to the
        // departing slot before reconciling the incoming page into a fresh
        // child. Pop already leaves the outgoing child at this exact index.
        if matches!(self.pending_transition, Some((_, TransitionKind::Replace)))
            && self.departing.is_some()
            && tree.children.len() == self.pages.len()
            && !self.pages.is_empty()
        {
            tree.children.push(Tree::empty());
            let incoming = self.pages.len() - 1;
            let outgoing = self.pages.len();
            tree.children.swap(incoming, outgoing);
        }

        while tree.children.len() < visual_len {
            tree.children.push(Tree::empty());
        }
        tree.children.truncate(visual_len);

        for (child, page) in tree.children.iter_mut().zip(self.pages.iter_mut()) {
            child.diff(page.element.as_widget_mut());
        }

        if let Some(departing) = self.departing.as_mut()
            && let Some(child) = tree.children.get_mut(self.pages.len())
        {
            child.diff(departing.element.as_widget_mut());
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let limits = limits.width(self.width).height(self.height);
        let size = limits.max();

        let mut children = Vec::new();
        for (i, page) in self.pages.iter_mut().enumerate() {
            if i < tree.children.len() {
                let child_layout =
                    page.element
                        .as_widget_mut()
                        .layout(&mut tree.children[i], renderer, &limits);
                children.push(child_layout);
            }
        }

        if let Some(departing) = self.departing.as_mut() {
            let index = self.pages.len();
            if index < tree.children.len() {
                children.push(departing.element.as_widget_mut().layout(
                    &mut tree.children[index],
                    renderer,
                    &limits,
                ));
            }
        }

        layout::Node::with_children(size, children)
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
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            tree.mark_dirty(tree::DirtyFlags::PAINT);
            let mut remove_departing = false;

            {
                let state = tree.state.downcast_mut::<NavigatorState>();
                state.last_instant = *now;

                if let Some((transition, kind)) = self.pending_transition.take() {
                    state.active_transition = Some(TransitionAnim::new(transition, kind, *now));
                }

                if let Some(ref mut anim) = state.active_transition {
                    if anim.advance(*now) {
                        shell.request_animation();
                        shell.request_redraw();
                    } else {
                        remove_departing =
                            matches!(anim.kind, TransitionKind::Pop | TransitionKind::Replace);
                        state.active_transition = None;
                    }
                }
            }

            if remove_departing && self.departing.take().is_some() {
                tree.children.truncate(self.pages.len());
                shell.invalidate_layout();
            }
        }

        // Forward events to the topmost page only
        if let Some(top) = self.pages.len().checked_sub(1) {
            for (i, child_layout) in layout.children().enumerate() {
                if i == top && i < tree.children.len() {
                    self.pages[i].element.as_widget_mut().update(
                        &mut tree.children[i],
                        event,
                        child_layout,
                        cursor,
                        renderer,
                        shell,
                        viewport,
                    );
                    break;
                }
            }
        }
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
        let state = tree.state.downcast_ref::<NavigatorState>();
        let children: Vec<_> = layout.children().collect();
        let transition = state.active_transition.as_ref();
        let now = state.last_instant;

        let visual_len = self.pages.len() + usize::from(self.departing.is_some());

        for i in 0..visual_len {
            if i >= children.len() || i >= tree.children.len() {
                break;
            }

            let page = if i < self.pages.len() {
                &self.pages[i]
            } else {
                self.departing
                    .as_ref()
                    .expect("visual departing page must exist")
            };
            let child_layout = children[i];

            // A transition animates between two pages: the entering
            // (topmost) and the exiting (second from top). Pages below
            // the pair are not drawn. A transition requires >= 2 pages.
            let transition_is_valid = transition.is_some_and(|anim| match anim.kind {
                TransitionKind::Push => self.pages.len() >= 2,
                TransitionKind::Pop | TransitionKind::Replace => self.departing.is_some(),
            });

            if let Some(anim) = transition.filter(|_| transition_is_valid) {
                let entering_idx = self.pages.len() - 1;
                let exiting_idx = match anim.kind {
                    TransitionKind::Push => self.pages.len() - 2,
                    TransitionKind::Pop | TransitionKind::Replace => self.pages.len(),
                };

                if i == entering_idx {
                    let t = anim.enter_state(now);
                    if t.opacity > 0.001 {
                        let bounds = child_layout.bounds();
                        let center_x = bounds.x + bounds.width / 2.0;
                        let center_y = bounds.y + bounds.height / 2.0;
                        let transform = core::Transformation::translate(
                            t.offset_x * bounds.width,
                            t.offset_y * bounds.height,
                        ) * core::Transformation::translate(center_x, center_y)
                            * core::Transformation::scale(t.scale)
                            * core::Transformation::translate(-center_x, -center_y);

                        renderer.with_opacity(t.opacity, |renderer| {
                            renderer.with_transformation(transform, |renderer| {
                                page.element.as_widget().draw(
                                    &tree.children[i],
                                    renderer,
                                    theme,
                                    style,
                                    child_layout,
                                    cursor,
                                    viewport,
                                );
                            });
                        });
                    }
                } else if i == exiting_idx {
                    let t = anim.exit_state(now);
                    if t.opacity > 0.001 {
                        let bounds = child_layout.bounds();
                        let center_x = bounds.x + bounds.width / 2.0;
                        let center_y = bounds.y + bounds.height / 2.0;
                        let transform = core::Transformation::translate(
                            t.offset_x * bounds.width,
                            t.offset_y * bounds.height,
                        ) * core::Transformation::translate(center_x, center_y)
                            * core::Transformation::scale(t.scale)
                            * core::Transformation::translate(-center_x, -center_y);

                        renderer.with_opacity(t.opacity, |renderer| {
                            renderer.with_transformation(transform, |renderer| {
                                page.element.as_widget().draw(
                                    &tree.children[i],
                                    renderer,
                                    theme,
                                    style,
                                    child_layout,
                                    cursor,
                                    viewport,
                                );
                            });
                        });
                    }
                }
                // Pages below the transition pair are not drawn.
            } else if i == self.pages.len().saturating_sub(1) {
                // No transition — draw only the top page.
                page.element.as_widget().draw(
                    &tree.children[i],
                    renderer,
                    theme,
                    style,
                    child_layout,
                    cursor,
                    viewport,
                );
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if let Some(top) = self.pages.len().checked_sub(1) {
            let children: Vec<_> = layout.children().collect();
            if let (Some(child_layout), Some(child_tree)) =
                (children.get(top), tree.children.get(top))
            {
                return self.pages[top].element.as_widget().mouse_interaction(
                    child_tree,
                    *child_layout,
                    cursor,
                    viewport,
                    renderer,
                );
            }
        }
        mouse::Interaction::default()
    }
}

impl<'a, Message, Theme, Renderer> From<Navigator<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: core::Renderer + 'a,
{
    fn from(navigator: Navigator<'a, Message, Theme, Renderer>) -> Self {
        Self::new(navigator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Root;
    struct Outgoing;
    struct Incoming;

    macro_rules! page_widget {
        ($page:ty) => {
            impl Widget<(), (), ()> for $page {
                fn size(&self) -> Size<Length> {
                    Size::new(Length::Fill, Length::Fill)
                }

                fn layout(
                    &mut self,
                    _tree: &mut Tree,
                    _renderer: &(),
                    limits: &layout::Limits,
                ) -> layout::Node {
                    layout::Node::new(limits.max())
                }

                fn draw(
                    &self,
                    _tree: &Tree,
                    _renderer: &mut (),
                    _theme: &(),
                    _style: &renderer::Style,
                    _layout: Layout<'_>,
                    _cursor: mouse::Cursor,
                    _viewport: &Rectangle,
                ) {
                }
            }
        };
    }

    page_widget!(Root);
    page_widget!(Outgoing);
    page_widget!(Incoming);

    #[test]
    fn pop_keeps_the_outgoing_page_until_the_reverse_transition_finishes() {
        let navigator: Navigator<'_, (), (), ()> = Navigator::new(Element::new(Root))
            .push(Element::new(Outgoing), PageTransition::FORWARD)
            .pop(PageTransition::BACK);

        assert_eq!(navigator.pages.len(), 1);
        assert!(navigator.departing.is_some());
        assert!(matches!(
            navigator.pending_transition,
            Some((PageTransition::SlideRight { .. }, TransitionKind::Pop))
        ));
    }

    #[test]
    fn replace_moves_the_existing_top_state_to_the_departing_slot() {
        let mut previous: Navigator<'_, (), (), ()> =
            Navigator::new(Element::new(Root)).push(Element::new(Outgoing), PageTransition::None);
        let mut tree = Tree::new(&previous as &dyn Widget<(), (), ()>);
        previous.diff(&mut tree);
        let outgoing_id = tree.children[1].id().clone();

        let mut replacement: Navigator<'_, (), (), ()> = Navigator::new(Element::new(Root))
            .push(Element::new(Outgoing), PageTransition::None)
            .replace(Element::new(Incoming), PageTransition::ZOOM);
        replacement.diff(&mut tree);

        assert_ne!(tree.children[1].id(), &outgoing_id);
        assert_eq!(tree.children[2].id(), &outgoing_id);
    }

    #[test]
    fn spring_navigation_is_animated_instead_of_becoming_an_instant_cut() {
        let now = Instant::now();
        let mut animation = TransitionAnim::new(
            PageTransition::SpringSlide {
                spring: taim_motion::SpringConfig::default(),
            },
            TransitionKind::Push,
            now,
        );

        assert!(animation.advance(now));
        assert!(animation.progress(now) < 1.0);
    }
}
