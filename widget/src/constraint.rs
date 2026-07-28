//! Clamp a child's size to explicit min/max/aspect-ratio limits, with an
//! optional diagnostic overlay showing when a limit actually binds.
use crate::core::layout::{self, Layout};
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::widget::visit;
use crate::core::widget::{Operation, Tree};
use crate::core::{
    Background, Color, Element, Event, Length, Rectangle, Shell, Size, Vector, Widget,
};

/// The min/max/aspect-ratio limits a [`Constraint`] widget applies to
/// its content, on top of whatever [`layout::Limits`] its parent already
/// imposes.
///
/// Every field is optional; an unset field simply does not further
/// restrict that dimension beyond what the parent already provides.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Limits {
    /// The minimum width, in pixels.
    pub min_width: Option<f32>,
    /// The maximum width, in pixels.
    pub max_width: Option<f32>,
    /// The minimum height, in pixels.
    pub min_height: Option<f32>,
    /// The maximum height, in pixels.
    pub max_height: Option<f32>,
    /// The required width-to-height ratio (width divided by height).
    ///
    /// When set, the resolved size is adjusted to satisfy this ratio
    /// exactly, shrinking whichever dimension the parent gave the most
    /// slack in -- the same convention `layout::flex`'s own
    /// aspect-driven paths (e.g. `Grid`'s `Sizing::AspectRatio`) use.
    pub aspect_ratio: Option<f32>,
}

impl Limits {
    /// No additional limits: a [`Constraint`] with this has no effect.
    pub const NONE: Self = Self {
        min_width: None,
        max_width: None,
        min_height: None,
        max_height: None,
        aspect_ratio: None,
    };

    /// Sets the minimum width.
    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = Some(width);
        self
    }

    /// Sets the maximum width.
    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = Some(width);
        self
    }

    /// Sets the minimum height.
    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = Some(height);
        self
    }

    /// Sets the maximum height.
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = Some(height);
        self
    }

    /// Sets the required aspect ratio (width / height).
    pub fn aspect_ratio(mut self, ratio: f32) -> Self {
        self.aspect_ratio = Some(ratio);
        self
    }

    /// Applies these [`Limits`] on top of `parent`, returning the
    /// resulting, tighter [`layout::Limits`] a child should be laid out
    /// against, along with a [`Diagnostic`] reporting whether each
    /// aspect actually narrowed `parent`'s own bounds.
    fn resolve(&self, parent: &layout::Limits) -> (layout::Limits, Diagnostic) {
        let original_max = parent.max();

        let mut min = parent.min();
        let mut max = original_max;

        if let Some(value) = self.min_width {
            min.width = min.width.max(value);
        }

        if let Some(value) = self.max_width {
            max.width = max.width.min(value);
        }

        // `min` may have just been raised above `max` (e.g. a
        // `min_width` larger than the parent's own `max.width`, with no
        // `max_width` override to raise `max` back in step) --
        // `layout::Limits` never validates `min <= max` itself, so an
        // inverted box here would silently propagate as a broken
        // constraint. Widening `max` to match is the only sound choice:
        // a requested minimum takes priority over a parent's maximum,
        // exactly as a `Length::Fixed` bigger than its parent's limits
        // already does elsewhere in `layout::Limits::width`/`height`.
        max.width = max.width.max(min.width);

        if let Some(value) = self.min_height {
            min.height = min.height.max(value);
        }

        if let Some(value) = self.max_height {
            max.height = max.height.min(value);
        }

        max.height = max.height.max(min.height);

        // The aspect ratio is a relationship between width and height
        // that `layout::Limits` alone cannot express (it only bounds
        // each axis independently), so it must be resolved into a
        // concrete `max` *before* the child is laid out -- fixing it up
        // after the fact would leave the child's own returned `Node`
        // reporting a size inconsistent with what this widget reports
        // to its own parent.
        let aspect_applied = if let Some(ratio) = self.aspect_ratio {
            let ratio_height = max.width / ratio;

            if ratio_height <= max.height {
                max.height = ratio_height;
            } else {
                max.width = max.height * ratio;
            }

            min.width = min.width.min(max.width);
            min.height = min.height.min(max.height);

            true
        } else {
            false
        };

        let diagnostic = Diagnostic {
            width_bound: max.width != original_max.width,
            height_bound: max.height != original_max.height,
            aspect_applied,
        };

        (layout::Limits::new(min, max), diagnostic)
    }
}

/// Whether a [`Constraint`]'s limits actually changed the resolved size
/// on a given axis, for [`Constraint::diagnostic`]'s overlay.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Diagnostic {
    width_bound: bool,
    height_bound: bool,
    aspect_applied: bool,
}

/// A widget that clamps its content's size to explicit [`Limits`].
pub struct Constraint<'a, Message, Theme = crate::Theme, Renderer = crate::Renderer>
where
    Renderer: crate::core::Renderer,
{
    content: Element<'a, Message, Theme, Renderer>,
    limits: Limits,
    diagnostic: bool,
}

impl<'a, Message, Theme, Renderer> Constraint<'a, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    /// Wraps `content`, applying `limits` to its resolved size.
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>, limits: Limits) -> Self {
        Self {
            content: content.into(),
            limits,
            diagnostic: false,
        }
    }

    /// Enables a diagnostic overlay: whenever a limit actually binds
    /// (the content would otherwise have been a different size), a thin
    /// colored border is drawn on the bound edge(s) -- red for a
    /// min/max clamp, blue for the aspect ratio -- so a developer can
    /// see at a glance which constraint is actually driving a
    /// surprising size, without reaching for an external inspector.
    ///
    /// Meant for development; leave disabled (the default) in shipped
    /// UI.
    pub fn diagnostic(mut self, enabled: bool) -> Self {
        self.diagnostic = enabled;
        self
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Constraint<'_, Message, Theme, Renderer>
where
    Renderer: crate::core::Renderer,
{
    fn tag(&self) -> crate::core::widget::tree::Tag {
        crate::core::widget::tree::Tag::of::<Diagnostic>()
    }

    fn state(&self) -> crate::core::widget::tree::State {
        crate::core::widget::tree::State::new(Diagnostic::default())
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let (child_limits, diagnostic) = self.limits.resolve(limits);

        *tree.state.downcast_mut::<Diagnostic>() = diagnostic;

        let content =
            self.content
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, &child_limits);

        let size = content.size();

        layout::Node::with_children(size, vec![content])
    }

    fn visit_children(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
        visit(&mut visit::Widgets::new(
            self.content.as_widget_mut(),
            &mut tree.children[0],
            layout.children().next().unwrap(),
        ));
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                layout.children().next().unwrap(),
                renderer,
                operation,
            );
        });
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
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout.children().next().unwrap(),
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
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout.children().next().unwrap(),
            cursor,
            viewport,
            renderer,
        )
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
        let content_layout = layout.children().next().unwrap();

        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            content_layout,
            cursor,
            viewport,
        );

        if self.diagnostic {
            let diagnostic = *tree.state.downcast_ref::<Diagnostic>();

            if diagnostic.width_bound || diagnostic.height_bound || diagnostic.aspect_applied {
                let color = if diagnostic.aspect_applied {
                    Color::from_rgb(0.2, 0.4, 1.0)
                } else {
                    Color::from_rgb(1.0, 0.2, 0.2)
                };

                renderer.fill_quad(
                    renderer::Quad {
                        bounds: layout.bounds(),
                        border: crate::core::Border {
                            color,
                            width: 2.0,
                            radius: 0.0.into(),
                        },
                        shadow: crate::core::Shadow::default(),
                        snap: false,
                    },
                    Background::Color(Color::TRANSPARENT),
                );
            }
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<Constraint<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: crate::core::Renderer + 'a,
{
    fn from(constraint: Constraint<'a, Message, Theme, Renderer>) -> Self {
        Self::new(constraint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parent(max: Size) -> layout::Limits {
        layout::Limits::new(Size::ZERO, max)
    }

    #[test]
    fn no_limits_never_bind() {
        let limits = Limits::NONE;
        let (resolved, diagnostic) = limits.resolve(&parent(Size::new(100.0, 50.0)));

        assert_eq!(resolved.max(), Size::new(100.0, 50.0));
        assert!(!diagnostic.width_bound);
        assert!(!diagnostic.height_bound);
    }

    #[test]
    fn min_width_raises_max_when_the_parent_is_narrower() {
        let limits = Limits::NONE.min_width(150.0);
        let (resolved, diagnostic) = limits.resolve(&parent(Size::new(100.0, 50.0)));

        // `layout::Limits` never allows `min > max`, so a `min_width`
        // exceeding the parent's own `max.width` must widen `max` too,
        // rather than silently producing an inverted box.
        assert_eq!(resolved.min().width, 150.0);
        assert_eq!(resolved.max().width, 150.0);
        assert!(
            diagnostic.width_bound,
            "widening `max` to satisfy `min_width` is itself a binding change worth flagging"
        );
    }

    #[test]
    fn min_width_below_the_parents_max_does_not_bind() {
        let limits = Limits::NONE.min_width(50.0);
        let (resolved, diagnostic) = limits.resolve(&parent(Size::new(100.0, 50.0)));

        assert_eq!(resolved.min().width, 50.0);
        assert_eq!(resolved.max().width, 100.0);
        assert!(!diagnostic.width_bound);
    }

    #[test]
    fn max_width_clamps_the_maximum_down() {
        let limits = Limits::NONE.max_width(50.0);
        let (resolved, diagnostic) = limits.resolve(&parent(Size::new(100.0, 50.0)));

        assert_eq!(resolved.max().width, 50.0);
        assert!(diagnostic.width_bound);
    }

    #[test]
    fn min_and_max_height_both_apply() {
        let limits = Limits::NONE.min_height(20.0).max_height(30.0);

        let (widened, _) = limits.resolve(&parent(Size::new(10.0, 10.0)));
        assert_eq!(widened.max().height, 20.0);

        let (narrowed, diagnostic) = limits.resolve(&parent(Size::new(10.0, 50.0)));
        assert_eq!(narrowed.max().height, 30.0);
        assert!(diagnostic.height_bound);
    }

    #[test]
    fn aspect_ratio_shrinks_the_taller_dimension() {
        let limits = Limits::NONE.aspect_ratio(2.0);
        let (resolved, diagnostic) = limits.resolve(&parent(Size::new(100.0, 100.0)));

        assert_eq!(resolved.max(), Size::new(100.0, 50.0));
        assert!(diagnostic.aspect_applied);
    }

    #[test]
    fn aspect_ratio_shrinks_the_wider_dimension() {
        let limits = Limits::NONE.aspect_ratio(0.5);
        let (resolved, diagnostic) = limits.resolve(&parent(Size::new(100.0, 100.0)));

        assert_eq!(resolved.max(), Size::new(50.0, 100.0));
        assert!(diagnostic.aspect_applied);
    }

    #[test]
    fn aspect_ratio_is_a_noop_when_already_satisfied() {
        let limits = Limits::NONE.aspect_ratio(2.0);
        let (resolved, _) = limits.resolve(&parent(Size::new(100.0, 50.0)));

        assert_eq!(resolved.max(), Size::new(100.0, 50.0));
    }
}
