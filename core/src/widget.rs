//! Create custom widgets and operate on them.
pub mod operation;
pub mod text;
pub mod tree;
pub mod visit;

mod id;

pub use id::Id;
pub use operation::Operation;
pub use text::Text;
pub use tree::Tree;
pub use visit::Visit;

use crate::layout::{self, Layout};
use crate::mouse;
use crate::overlay;
use crate::renderer;
use crate::{Event, Length, Point, Rectangle, Shell, Size, Vector};
use crate::{cache, semantics};

/// A component that displays information and allows interaction.
///
/// If you want to build your own widgets, you will need to implement this
/// trait.
///
/// # Examples
/// The repository has some [examples] showcasing how to implement a custom
/// widget:
///
/// - [`custom_widget`], a demonstration of how to build a custom widget that
///   draws a circle.
/// - [`geometry`], a custom widget showcasing how to draw geometry with the
///   `Mesh2D` primitive in [`aura_wgpu`].
///
/// [examples]: https://github.com/theaimartco/vyom-aura/tree/master/examples
/// [`custom_widget`]: https://github.com/theaimartco/vyom-aura/tree/master/examples/custom_widget
/// [`geometry`]: https://github.com/theaimartco/vyom-aura/tree/master/examples/geometry
/// [`aura_wgpu`]: https://github.com/theaimartco/vyom-aura/tree/master/wgpu
pub trait Widget<Message, Theme, Renderer>
where
    Renderer: crate::Renderer,
{
    /// Returns the [`Size`] of the [`Widget`] in lengths.
    fn size(&self) -> Size<Length>;

    /// Returns the [`layout::Node`] of the [`Widget`].
    ///
    /// This [`layout::Node`] is used by the runtime to compute the [`Layout`] of the
    /// user interface.
    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node;

    /// Draws the [`Widget`] using the associated `Renderer`.
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    );

    /// Returns the [`Tag`] of the [`Widget`].
    ///
    /// [`Tag`]: tree::Tag
    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    /// Returns the [`State`] of the [`Widget`].
    ///
    /// [`State`]: tree::State
    fn state(&self) -> tree::State {
        tree::State::None
    }

    /// Reconciles the [`Widget`] with the provided [`Tree`].
    fn diff(&mut self, tree: &mut Tree) {
        tree.children.clear();
    }

    /// Applies an [`Operation`] to the [`Widget`].
    fn operate(
        &mut self,
        _tree: &mut Tree,
        _layout: Layout<'_>,
        _renderer: &Renderer,
        _operation: &mut dyn Operation,
    ) {
    }

    /// Processes a runtime [`Event`].
    ///
    /// By default, it does nothing.
    fn update(
        &mut self,
        _tree: &mut Tree,
        _event: &Event,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        _shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
    }

    /// Returns the current [`mouse::Interaction`] of the [`Widget`].
    ///
    /// By default, it returns [`mouse::Interaction::None`].
    fn mouse_interaction(
        &self,
        _tree: &Tree,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        mouse::Interaction::None
    }

    /// Returns the overlay of the [`Widget`], if there is any.
    fn overlay<'a>(
        &'a mut self,
        _tree: &'a mut Tree,
        _layout: Layout<'a>,
        _renderer: &Renderer,
        _viewport: &Rectangle,
        _translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        None
    }

    /// Returns whether the [`Widget`] is [`Void`].
    fn is_void(&self) -> bool {
        false
    }

    /// Returns the bounds a change in this [`Widget`]'s appearance would
    /// damage, given its `layout`.
    ///
    /// The default implementation conservatively returns the full
    /// `layout` bounds, which is always correct (if potentially
    /// wasteful): any real content this [`Widget`] draws is contained
    /// within its own layout bounds. A [`Widget`] that draws outside its
    /// bounds (e.g. a shadow or an overflowing tooltip) must override
    /// this to include that area, or later phases' damage-driven
    /// redraw (Phase 4) will clip it incorrectly.
    fn paint_bounds(&self, _tree: &Tree, layout: Layout<'_>) -> Rectangle {
        layout.bounds()
    }

    /// Returns whether the given `point` should be considered part of
    /// this [`Widget`] for hit-testing purposes, given its `layout`.
    ///
    /// The default implementation is the conservative
    /// rectangular-bounds check every [`Widget`] already satisfies by
    /// construction. A [`Widget`] with a non-rectangular hit area (e.g.
    /// a circular button) may override this for pixel-accurate hit
    /// testing; until then, the linear/full-tree oracle described in
    /// `phase.md` Phase 3 can rely on this default matching the
    /// bounds-based hit testing every widget currently receives via
    /// [`Layout`], so introducing this method changes no observable
    /// behavior yet.
    fn hit_test(&self, _tree: &Tree, layout: Layout<'_>, point: Point) -> bool {
        layout.bounds().contains(point)
    }

    /// Returns the [`semantics::Node`] describing this [`Widget`] to
    /// assistive technology, if it has one.
    ///
    /// This is a [`Widget`]'s **own**, non-recursive node: it must not
    /// include children. Recursion is [`Widget::semantics_children`]'s
    /// job, so that a clean subtree can contribute a cached node without
    /// this method being called at all (see
    /// [`Tree::semantics_with_cache`]).
    ///
    /// The default implementation returns [`None`]: a [`Widget`] that
    /// says nothing to assistive technology is invisible to it, which is
    /// correct for a purely presentational one (a `rule`, a decorative
    /// `image`) and a bug for anything a user can perceive or operate.
    ///
    /// [`Tree::semantics_with_cache`]: tree::Tree::semantics_with_cache
    fn semantics(&self, _tree: &Tree, _layout: Layout<'_>) -> Option<semantics::Node> {
        None
    }

    /// Visits each of this [`Widget`]'s children with the `Tree` and
    /// [`Layout`] belonging to it.
    ///
    /// This exists because a [`Widget`] cannot be asked for its children
    /// generically: every one stores them differently (a `Vec<Element>`,
    /// a single `Element`, an `Option<Element>`, none at all), which is
    /// why every other traversal in this trait — [`diff`], [`layout`],
    /// [`draw`], [`update`], [`operate`] — is likewise driven by the
    /// [`Widget`] itself rather than by the caller. Whole-tree traversals
    /// that never name `Message`/`Theme`/`Renderer` — accessibility
    /// assembly and hit-testing — need the same thing, and this is it.
    ///
    /// The default implementation visits nothing, which is correct for a
    /// leaf. **A [`Widget`] with children must override this**, or its
    /// entire subtree becomes invisible to assistive technology *and*
    /// unreachable by hit-testing — the visitor has no other way to reach
    /// it, and nothing fails loudly. The body is the same zip over
    /// `children`/`tree.children`/`layout.children()` that this
    /// [`Widget`]'s [`operate`] already performs; keep the two in step.
    ///
    /// [`diff`]: Self::diff
    /// [`layout`]: Self::layout
    /// [`draw`]: Self::draw
    /// [`update`]: Self::update
    /// [`operate`]: Self::operate
    /// Each child is handed over as a [`visit::Widgets`] — a
    /// [`visit::Visit`], not a `&mut dyn Widget` — because a widget
    /// reference cannot cross a theme boundary (`themer` is a
    /// `Widget<Message, AnyTheme, Renderer>` wrapping a `Widget<Message,
    /// Theme, Renderer>`). See [`visit::Visit`] for the full reason.
    ///
    /// This one hook serves *every* whole-tree traversal that does not
    /// name `Message`/`Theme`/`Renderer` — accessibility assembly and
    /// hit-testing today. A second, parallel hook would mean writing this
    /// same zip twice per container with nothing keeping the copies in
    /// step.
    ///
    /// Takes `&mut self`, as [`operate`] does and for the same reason: a
    /// `lazy` or `responsive` [`Widget`] must *build* its children before
    /// it can hand them over, and cannot do so through a shared borrow.
    ///
    /// [`operate`]: Self::operate
    fn visit_children(
        &mut self,
        _tree: &mut Tree,
        _layout: Layout<'_>,
        _visit: &mut dyn FnMut(&mut dyn visit::Visit),
    ) {
    }

    /// Returns the [`cache::Policy`] this [`Widget`] would like applied
    /// to any cache it participates in (e.g. a retained layer, Phase 4;
    /// a shaped-text cache, Phase 10).
    ///
    /// The default implementation returns [`None`], meaning "no opinion,
    /// use the cache's own default policy." Most widgets should never
    /// need to override this; it exists for widgets whose content is
    /// unusually cheap or expensive to reproduce (e.g. a widget that
    /// decodes a large image benefits from [`cache::Class::Retained`]
    /// even when scrolled off-screen, since redecoding is far more
    /// expensive than keeping the bytes around).
    fn cache_policy(&self) -> Option<cache::Policy> {
        None
    }
}

/// A zero-sized [`Widget`] that does nothing and will be filtered out by containers.
pub struct Void;

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Void
where
    Renderer: crate::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Shrink,
            height: Length::Shrink,
        }
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        _limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(Size::ZERO)
    }

    fn draw(
        &self,
        _tree: &Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }

    fn is_void(&self) -> bool {
        true
    }
}
