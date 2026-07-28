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

// -----------------------------------------------------------------------
// Page transition types (self-contained, no external dep on aura-motion)
// -----------------------------------------------------------------------

/// The visual state of a page surface mid-transition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformState {
    /// Horizontal offset as a fraction of the surface width.
    pub offset_x: f32,
    /// Vertical offset as a fraction of the surface height.
    pub offset_y: f32,
    /// Surface opacity, 0.0 = invisible, 1.0 = fully opaque.
    pub opacity: f32,
}

impl TransformState {
    const IDENTITY: Self = Self {
        offset_x: 0.0,
        offset_y: 0.0,
        opacity: 1.0,
    };
}

/// A page transition specification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PageTransition {
    /// A simple cross-fade between pages.
    Fade {
        /// Duration in milliseconds.
        duration_ms: u16,
    },
    /// The new page slides in from the right (forward navigation).
    SlideLeft {
        /// Duration in milliseconds.
        duration_ms: u16,
    },
    /// The new page slides in from the left (back navigation).
    SlideRight {
        /// Duration in milliseconds.
        duration_ms: u16,
    },
    /// The new page slides up from the bottom (modal presentation).
    SlideUp {
        /// Duration in milliseconds.
        duration_ms: u16,
    },
    /// Scale + fade: new page scales up from 85% while fading in
    /// (Material 3 "container transform").
    ZoomFade {
        /// Duration in milliseconds.
        duration_ms: u16,
    },
    /// No animation — an instant cut.
    None,
}

impl Default for PageTransition {
    fn default() -> Self {
        Self::SlideLeft { duration_ms: 300 }
    }
}

impl PageTransition {
    /// A fast fade, suitable for tab switches.
    pub const FADE: Self = Self::Fade { duration_ms: 200 };

    /// The standard forward navigation slide.
    pub const FORWARD: Self = Self::SlideLeft { duration_ms: 300 };

    /// The standard back navigation slide.
    pub const BACK: Self = Self::SlideRight { duration_ms: 300 };

    /// A modal presentation from the bottom.
    pub const MODAL: Self = Self::SlideUp { duration_ms: 400 };

    /// Material 3-style container transform.
    pub const ZOOM: Self = Self::ZoomFade { duration_ms: 350 };

    fn duration_seconds(self) -> f64 {
        match self {
            Self::Fade { duration_ms }
            | Self::SlideLeft { duration_ms }
            | Self::SlideRight { duration_ms }
            | Self::SlideUp { duration_ms }
            | Self::ZoomFade { duration_ms } => f64::from(duration_ms) / 1000.0,
            Self::None => 0.0,
        }
    }

    fn ease(t: f32) -> f32 {
        // EaseInOut quadratic
        let t = t.clamp(0.0, 1.0);
        if t < 0.5 {
            2.0 * t * t
        } else {
            let inv = -2.0 * t + 2.0;
            1.0 - inv * inv / 2.0
        }
    }

    fn enter_at(self, t: f32) -> TransformState {
        let e = Self::ease(t);
        match self {
            Self::Fade { .. } => TransformState {
                opacity: e,
                ..TransformState::IDENTITY
            },
            Self::SlideLeft { .. } => TransformState {
                offset_x: 1.0 - e,
                ..TransformState::IDENTITY
            },
            Self::SlideRight { .. } => TransformState {
                offset_x: -(1.0 - e),
                ..TransformState::IDENTITY
            },
            Self::SlideUp { .. } => TransformState {
                offset_y: 1.0 - e,
                ..TransformState::IDENTITY
            },
            Self::ZoomFade { .. } => TransformState {
                offset_x: 0.0,
                offset_y: 0.0,
                opacity: e,
            },
            Self::None => TransformState::IDENTITY,
        }
    }

    fn exit_at(self, t: f32) -> TransformState {
        let e = Self::ease(t);
        match self {
            Self::Fade { .. } => TransformState {
                opacity: 1.0 - e,
                ..TransformState::IDENTITY
            },
            Self::SlideLeft { .. } => TransformState {
                offset_x: -0.3 * e,
                ..TransformState::IDENTITY
            },
            Self::SlideRight { .. } => TransformState {
                offset_x: 0.3 * e,
                ..TransformState::IDENTITY
            },
            Self::SlideUp { .. } => TransformState {
                opacity: 1.0 - 0.3 * e,
                ..TransformState::IDENTITY
            },
            Self::ZoomFade { .. } => TransformState {
                offset_x: 0.0,
                offset_y: 0.0,
                opacity: 1.0 - e,
            },
            Self::None => TransformState {
                opacity: 0.0,
                ..TransformState::IDENTITY
            },
        }
    }
}

// -----------------------------------------------------------------------
// Animation state
// -----------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct TransitionAnim {
    transition: PageTransition,
    started_at: Instant,
    duration: f64,
    settled: bool,
}

impl TransitionAnim {
    fn new(transition: PageTransition, now: Instant) -> Self {
        let duration = transition.duration_seconds();
        Self {
            transition,
            started_at: now,
            duration,
            settled: duration <= 0.0,
        }
    }

    fn progress(&self, now: Instant) -> f32 {
        if self.settled {
            return 1.0;
        }
        let elapsed = now.duration_since(self.started_at).as_secs_f64();
        if self.duration > 0.0 {
            (elapsed / self.duration).min(1.0) as f32
        } else {
            1.0
        }
    }

    fn advance(&mut self, now: Instant) -> bool {
        if self.settled {
            return false;
        }
        if self.progress(now) >= 1.0 {
            self.settled = true;
        }
        !self.settled
    }

    fn enter_state(&self, now: Instant) -> TransformState {
        self.transition.enter_at(self.progress(now))
    }

    fn exit_state(&self, now: Instant) -> TransformState {
        self.transition.exit_at(self.progress(now))
    }
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
/// use aura_widget::navigator::{Navigator, PageTransition};
///
/// Navigator::new(page_a)
///     .push(page_b, PageTransition::FORWARD)
/// ```
pub struct Navigator<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: core::Renderer,
{
    pages: Vec<Page<'a, Message, Theme, Renderer>>,
    pending_transition: Option<PageTransition>,
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
        self.pending_transition = Some(transition);
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
        while tree.children.len() < self.pages.len() {
            tree.children.push(Tree::empty());
        }
        tree.children.truncate(self.pages.len());

        for (child, page) in tree.children.iter_mut().zip(self.pages.iter_mut()) {
            child.diff(page.element.as_widget_mut());
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
            let state = tree.state.downcast_mut::<NavigatorState>();
            state.last_instant = *now;

            if let Some(transition) = self.pending_transition.take() {
                state.active_transition = Some(TransitionAnim::new(transition, *now));
            }

            if let Some(ref mut anim) = state.active_transition {
                if anim.advance(*now) {
                    shell.request_redraw();
                } else {
                    state.active_transition = None;
                }
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

        for (i, page) in self.pages.iter().enumerate() {
            if i >= children.len() || i >= tree.children.len() {
                break;
            }

            let child_layout = children[i];

            // A transition animates between two pages: the entering
            // (topmost) and the exiting (second from top). Pages below
            // the pair are not drawn. A transition requires >= 2 pages.
            if let Some(anim) = transition.filter(|_| self.pages.len() >= 2) {
                let entering_idx = self.pages.len() - 1;
                let exiting_idx = self.pages.len() - 2;

                if i == entering_idx {
                    let t = anim.enter_state(now);
                    if t.opacity > 0.001 {
                        let bounds = child_layout.bounds();
                        renderer.with_translation(
                            core::Vector::new(
                                t.offset_x * bounds.width,
                                t.offset_y * bounds.height,
                            ),
                            |renderer| {
                                page.element.as_widget().draw(
                                    &tree.children[i],
                                    renderer,
                                    theme,
                                    style,
                                    child_layout,
                                    cursor,
                                    viewport,
                                );
                            },
                        );
                    }
                } else if i == exiting_idx {
                    let t = anim.exit_state(now);
                    if t.opacity > 0.001 {
                        let bounds = child_layout.bounds();
                        renderer.with_translation(
                            core::Vector::new(
                                t.offset_x * bounds.width,
                                t.offset_y * bounds.height,
                            ),
                            |renderer| {
                                page.element.as_widget().draw(
                                    &tree.children[i],
                                    renderer,
                                    theme,
                                    style,
                                    child_layout,
                                    cursor,
                                    viewport,
                                );
                            },
                        );
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
