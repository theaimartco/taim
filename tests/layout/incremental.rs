//! Constraint/revision matrix and clean-subtree assertions.
//!
//! This file proves the exit gate stated in `phase.md` Phase 3:
//! "changing one leaf in a 10,000-node synthetic tree lays out/paints
//! only the required path/subtree." It builds a synthetic tree of
//! `Leaf`/`Branch` test widgets that cooperate with
//! `taim_core::widget::tree::Tree::layout_with_cache` (added in this
//! phase's second commit), instruments every leaf's layout call with a
//! shared counter, and asserts that re-laying-out the tree after
//! touching exactly one leaf only re-measures that leaf and its
//! ancestors -- never a sibling subtree.
//!
//! This does **not** prove that taim's shipped, production widgets
//! (`Column`, `Row`, `Container`, ...) get this speedup today -- none of
//! them call `layout_with_cache` yet, since retrofitting every widget is
//! out of this phase's scope (see `docs/architecture/widget-lifecycle.md`).
//! It proves the mechanism itself is correct and safe to adopt.

type Element<'a, M, T = taim_core::Theme, R = taim_renderer::Renderer> =
    taim_core::Element<'a, M, T, R>;
use taim_core::widget::{self, Widget};
use taim_core::{Layout, mouse, renderer};
use taim_core::{Length, Point, Rectangle, Size, Theme};
use taim_renderer::Renderer;

use std::cell::RefCell;
use std::rc::Rc;

/// Shared, process-wide-per-test counters recording how many times
/// `layout` actually ran for each leaf, keyed by the leaf's index.
#[derive(Debug, Default, Clone)]
struct LayoutCounters(Rc<RefCell<Vec<u32>>>);

impl LayoutCounters {
    fn new(leaf_count: usize) -> Self {
        Self(Rc::new(RefCell::new(vec![0; leaf_count])))
    }

    fn record(&self, leaf_index: usize) {
        self.0.borrow_mut()[leaf_index] += 1;
    }

    fn calls(&self, leaf_index: usize) -> u32 {
        self.0.borrow()[leaf_index]
    }

    fn total_calls(&self) -> u32 {
        self.0.borrow().iter().sum()
    }
}

/// A leaf widget with a fixed size, whose `layout` call is counted.
struct Leaf {
    index: usize,
    size: f32,
    counters: LayoutCounters,
}

impl<Message> Widget<Message, Theme, Renderer> for Leaf {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.size), Length::Fixed(self.size))
    }

    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<usize>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(self.index)
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        _renderer: &Renderer,
        limits: &taim_core::layout::Limits,
    ) -> taim_core::layout::Node {
        let counters = self.counters.clone();
        let index = self.index;
        let size = self.size;

        tree.layout_with_cache(*limits, move |_tree| {
            counters.record(index);
            taim_core::layout::Node::new(Size::new(size, size))
        })
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }
}

/// A branch widget that stacks its children vertically, forwarding
/// layout to each child's own cached-or-fresh measurement. `Branch`
/// itself always recomputes its own aggregate size (summing children),
/// since composing already-cheap cached results is trivial; the
/// expensive, avoidable work this test measures lives entirely in
/// `Leaf::layout`.
struct Branch<Message> {
    children: Vec<Element<'static, Message>>,
}

impl<Message> Widget<Message, Theme, Renderer> for Branch<Message> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<()>()
    }

    fn diff(&mut self, tree: &mut widget::Tree) {
        tree.diff_children(&mut self.children);
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &taim_core::layout::Limits,
    ) -> taim_core::layout::Node {
        if tree.children.len() != self.children.len() {
            tree.children = self
                .children
                .iter()
                .map(|child| widget::Tree::new(child.as_widget()))
                .collect();
        }

        let mut width = 0.0_f32;
        let mut height = 0.0_f32;

        let nodes: Vec<_> = self
            .children
            .iter_mut()
            .zip(tree.children.iter_mut())
            .map(|(child, child_tree)| {
                let node = child.as_widget_mut().layout(child_tree, renderer, limits);
                let size = node.size();

                width = width.max(size.width);
                height += size.height;

                node
            })
            .collect();

        let mut y = 0.0;
        let positioned: Vec<_> = nodes
            .into_iter()
            .map(|node| {
                let positioned = node.move_to(Point::new(0.0, y));
                y += positioned.bounds().height;
                positioned
            })
            .collect();

        taim_core::layout::Node::with_children(Size::new(width, height), positioned)
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }
}

const LEAF_COUNT: usize = 10_000;
const BRANCHING_FACTOR: usize = 100;

/// Builds a two-level tree of `BRANCHING_FACTOR` branches, each holding
/// `LEAF_COUNT / BRANCHING_FACTOR` leaves, for exactly `LEAF_COUNT`
/// leaves in total.
fn build_tree(counters: &LayoutCounters) -> Element<'static, ()> {
    let leaves_per_branch = LEAF_COUNT / BRANCHING_FACTOR;

    let branches: Vec<Element<'static, ()>> = (0..BRANCHING_FACTOR)
        .map(|branch_index| {
            let children: Vec<Element<'static, ()>> = (0..leaves_per_branch)
                .map(|leaf_in_branch| {
                    let index = branch_index * leaves_per_branch + leaf_in_branch;

                    Element::new(Leaf {
                        index,
                        size: 10.0,
                        counters: counters.clone(),
                    })
                })
                .collect();

            Element::new(Branch { children })
        })
        .collect();

    Element::new(Branch { children: branches })
}

fn headless_renderer() -> Renderer {
    taim_futures::futures::executor::block_on(<Renderer as taim_core::renderer::Headless>::new(
        taim_core::renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .expect("tiny-skia renderer must be available")
}

#[test]
fn changing_one_leaf_only_recomputes_that_leaf() {
    let counters = LayoutCounters::new(LEAF_COUNT);
    let renderer = headless_renderer();
    let limits = taim_core::layout::Limits::new(Size::ZERO, Size::new(2000.0, 200_000.0));

    let mut root = build_tree(&counters);
    let mut tree = widget::Tree::new(root.as_widget());
    tree.diff(root.as_widget_mut());

    let _ = root.as_widget_mut().layout(&mut tree, &renderer, &limits);

    assert_eq!(
        counters.total_calls(),
        LEAF_COUNT as u32,
        "the first layout pass must measure every leaf exactly once"
    );

    // Re-layout with completely unchanged limits and an unchanged tree:
    // every leaf should be able to reuse its cache.
    let _ = root.as_widget_mut().layout(&mut tree, &renderer, &limits);

    assert_eq!(
        counters.total_calls(),
        LEAF_COUNT as u32,
        "a clean re-layout with identical limits must not touch any leaf again"
    );

    // Mark exactly one leaf dirty, simulating "this leaf's content
    // changed" (the narrowest possible invalidation).
    let target_branch = 42;
    let target_leaf_in_branch = 7;
    let leaves_per_branch = LEAF_COUNT / BRANCHING_FACTOR;
    let target_index = target_branch * leaves_per_branch + target_leaf_in_branch;

    tree.children[target_branch].children[target_leaf_in_branch]
        .mark_dirty(widget::tree::DirtyFlags::LAYOUT);

    let _ = root.as_widget_mut().layout(&mut tree, &renderer, &limits);

    assert_eq!(
        counters.calls(target_index),
        2,
        "the touched leaf must be measured again"
    );

    for other_branch in 0..BRANCHING_FACTOR {
        for other_leaf_in_branch in 0..leaves_per_branch {
            if other_branch == target_branch && other_leaf_in_branch == target_leaf_in_branch {
                continue;
            }

            let other_index = other_branch * leaves_per_branch + other_leaf_in_branch;

            assert_eq!(
                counters.calls(other_index),
                1,
                "leaf {other_index} must not be re-measured; only leaf {target_index} was touched"
            );
        }
    }

    assert_eq!(
        counters.total_calls(),
        LEAF_COUNT as u32 + 1,
        "exactly one additional leaf measurement must have occurred across the whole tree"
    );
}

#[test]
fn different_limits_recompute_every_leaf_even_when_clean() {
    let counters = LayoutCounters::new(LEAF_COUNT);
    let renderer = headless_renderer();

    let mut root = build_tree(&counters);
    let mut tree = widget::Tree::new(root.as_widget());
    tree.diff(root.as_widget_mut());

    let limits_a = taim_core::layout::Limits::new(Size::ZERO, Size::new(2000.0, 200_000.0));
    let _ = root.as_widget_mut().layout(&mut tree, &renderer, &limits_a);

    assert_eq!(counters.total_calls(), LEAF_COUNT as u32);

    let limits_b = taim_core::layout::Limits::new(Size::ZERO, Size::new(1000.0, 100_000.0));
    let _ = root.as_widget_mut().layout(&mut tree, &renderer, &limits_b);

    assert_eq!(
        counters.total_calls(),
        (LEAF_COUNT as u32) * 2,
        "a changed set of limits invalidates every leaf's cache, since a Leaf's own \
         measurement does not depend on position within its parent -- only on the \
         limits it is directly given, which every leaf receives identically here"
    );
}

#[test]
fn a_leaf_marked_dirty_twice_is_only_measured_once_per_layout_pass() {
    let counters = LayoutCounters::new(LEAF_COUNT);
    let renderer = headless_renderer();
    let limits = taim_core::layout::Limits::new(Size::ZERO, Size::new(2000.0, 200_000.0));

    let mut root = build_tree(&counters);
    let mut tree = widget::Tree::new(root.as_widget());
    tree.diff(root.as_widget_mut());

    let _ = root.as_widget_mut().layout(&mut tree, &renderer, &limits);

    tree.children[0].children[0].mark_dirty(widget::tree::DirtyFlags::LAYOUT);
    tree.children[0].children[0].mark_dirty(widget::tree::DirtyFlags::LAYOUT);

    let _ = root.as_widget_mut().layout(&mut tree, &renderer, &limits);

    assert_eq!(counters.calls(0), 2);
}

// Phase 7 gives `Row`/`Column`/`Stack` their own persistent
// `layout::Cache` (see `widget/src/row.rs`'s internal `State`), closing
// part of the gap this file's own module doc called out: unlike Phase
// 3's `layout_with_cache` (still unused by production containers),
// these caches are wired into real, shipped widgets. The tests below
// prove that cache's two-sided contract: a `layout` call repeated
// without an intervening `diff` must reuse the cached result (skipping
// every child's own `layout`), while a `diff` -- modeling "content may
// have changed" -- must force a real recompute even under identical
// [`taim_core::layout::Limits`].

/// A leaf with no caching of its own -- unlike [`Leaf`] (which wraps
/// [`widget::Tree::layout_with_cache`] and so would mask a container's
/// own cache behind its own single-slot one), every call to
/// [`SimpleLeaf::layout`] always increments its counter, making it the
/// right instrument for testing whether a *container* (`Row`, `Column`,
/// `Stack`) actually calls a child's `layout` at all, rather than
/// whether the child itself happens to reuse a result internally.
struct SimpleLeaf {
    index: usize,
    size: f32,
    counters: LayoutCounters,
}

impl<Message> Widget<Message, Theme, Renderer> for SimpleLeaf {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.size), Length::Fixed(self.size))
    }

    fn layout(
        &mut self,
        _tree: &mut widget::Tree,
        _renderer: &Renderer,
        _limits: &taim_core::layout::Limits,
    ) -> taim_core::layout::Node {
        self.counters.record(self.index);

        taim_core::layout::Node::new(Size::new(self.size, self.size))
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }
}

fn leaf(index: usize, size: f32, counters: &LayoutCounters) -> Element<'static, ()> {
    Element::new(SimpleLeaf {
        index,
        size,
        counters: counters.clone(),
    })
}

#[test]
fn a_real_row_reuses_its_cache_across_layout_calls_with_no_diff_in_between() {
    use taim_widget::row;

    let counters = LayoutCounters::new(3);
    let renderer = headless_renderer();
    let limits = taim_core::layout::Limits::new(Size::ZERO, Size::new(500.0, 500.0));

    let mut element: Element<'static, ()> = row((0..3).map(|i| leaf(i, 10.0, &counters))).into();

    let mut tree = widget::Tree::new(element.as_widget());
    element.as_widget_mut().diff(&mut tree);

    let _ = element
        .as_widget_mut()
        .layout(&mut tree, &renderer, &limits);
    assert_eq!(
        counters.total_calls(),
        3,
        "the first layout pass must measure every child exactly once"
    );

    let _ = element
        .as_widget_mut()
        .layout(&mut tree, &renderer, &limits);
    assert_eq!(
        counters.total_calls(),
        3,
        "a second layout pass under identical limits, with no diff in between, must be \
         a cache hit and must not re-measure any child"
    );
}

#[test]
fn a_real_column_reuses_its_cache_across_layout_calls_with_no_diff_in_between() {
    use taim_widget::column;

    let counters = LayoutCounters::new(3);
    let renderer = headless_renderer();
    let limits = taim_core::layout::Limits::new(Size::ZERO, Size::new(500.0, 500.0));

    let mut element: Element<'static, ()> = column((0..3).map(|i| leaf(i, 10.0, &counters))).into();

    let mut tree = widget::Tree::new(element.as_widget());
    element.as_widget_mut().diff(&mut tree);

    let _ = element
        .as_widget_mut()
        .layout(&mut tree, &renderer, &limits);
    assert_eq!(counters.total_calls(), 3);

    let _ = element
        .as_widget_mut()
        .layout(&mut tree, &renderer, &limits);
    assert_eq!(
        counters.total_calls(),
        3,
        "a second layout pass under identical limits, with no diff in between, must be \
         a cache hit and must not re-measure any child"
    );
}

#[test]
fn a_row_recomputes_after_diff_even_under_identical_limits() {
    use taim_widget::row;

    let counters = LayoutCounters::new(3);
    let renderer = headless_renderer();
    let limits = taim_core::layout::Limits::new(Size::ZERO, Size::new(500.0, 500.0));

    let mut element: Element<'static, ()> = row((0..3).map(|i| leaf(i, 10.0, &counters))).into();

    let mut tree = widget::Tree::new(element.as_widget());
    element.as_widget_mut().diff(&mut tree);

    let _ = element
        .as_widget_mut()
        .layout(&mut tree, &renderer, &limits);
    assert_eq!(counters.total_calls(), 3);

    // A `diff` pass -- modeling a `view` re-run where a leaf's content
    // may have changed -- must invalidate the cache, even though the
    // `Limits` given to the next `layout` call are byte-for-byte
    // identical to before.
    element.as_widget_mut().diff(&mut tree);

    let _ = element
        .as_widget_mut()
        .layout(&mut tree, &renderer, &limits);
    assert_eq!(
        counters.total_calls(),
        6,
        "a diff pass must force every child to be re-measured on the next layout, since \
         Row/Column cannot yet observe fine-grained per-child content changes (see \
         Row::diff's own comment) and must conservatively assume any diff could invalidate \
         the cached measurement"
    );
}

#[test]
fn a_row_recomputes_when_limits_actually_change() {
    use taim_widget::row;

    let counters = LayoutCounters::new(3);
    let renderer = headless_renderer();

    let mut element: Element<'static, ()> = row((0..3).map(|i| leaf(i, 10.0, &counters))).into();

    let mut tree = widget::Tree::new(element.as_widget());
    element.as_widget_mut().diff(&mut tree);

    let limits_a = taim_core::layout::Limits::new(Size::ZERO, Size::new(500.0, 500.0));
    let _ = element
        .as_widget_mut()
        .layout(&mut tree, &renderer, &limits_a);
    assert_eq!(counters.total_calls(), 3);

    let limits_b = taim_core::layout::Limits::new(Size::ZERO, Size::new(300.0, 300.0));
    let _ = element
        .as_widget_mut()
        .layout(&mut tree, &renderer, &limits_b);
    assert_eq!(
        counters.total_calls(),
        6,
        "different limits must always recompute, even with no diff in between"
    );
}

#[test]
fn a_row_cache_survives_a_handful_of_recurring_limits_without_thrashing() {
    use taim_widget::row;

    let counters = LayoutCounters::new(3);
    let renderer = headless_renderer();

    let mut element: Element<'static, ()> = row((0..3).map(|i| leaf(i, 10.0, &counters))).into();

    let mut tree = widget::Tree::new(element.as_widget());
    element.as_widget_mut().diff(&mut tree);

    let limits_a = taim_core::layout::Limits::new(Size::ZERO, Size::new(500.0, 500.0));
    let limits_b = taim_core::layout::Limits::new(Size::ZERO, Size::new(300.0, 300.0));

    // Oscillate between two recurring limits (e.g. a resize settling
    // back and forth), with no diff in between -- the default capacity
    // (4) must be enough to keep both cached rather than thrashing to
    // zero reuse.
    for _ in 0..5 {
        let _ = element
            .as_widget_mut()
            .layout(&mut tree, &renderer, &limits_a);
        let _ = element
            .as_widget_mut()
            .layout(&mut tree, &renderer, &limits_b);
    }

    assert_eq!(
        counters.total_calls(),
        6,
        "only the first visit to each of the two recurring limits should ever measure \
         a child; every later visit to either one must be a cache hit"
    );
}

#[test]
fn a_real_stack_reuses_its_base_layer_cache_across_layout_calls_with_no_diff_in_between() {
    use taim_widget::stack;

    let counters = LayoutCounters::new(2);
    let renderer = headless_renderer();
    let limits = taim_core::layout::Limits::new(Size::ZERO, Size::new(500.0, 500.0));

    let mut element: Element<'static, ()> =
        stack([leaf(0, 50.0, &counters), leaf(1, 20.0, &counters)]).into();

    let mut tree = widget::Tree::new(element.as_widget());
    element.as_widget_mut().diff(&mut tree);

    let _ = element
        .as_widget_mut()
        .layout(&mut tree, &renderer, &limits);
    assert_eq!(counters.total_calls(), 2);

    let _ = element
        .as_widget_mut()
        .layout(&mut tree, &renderer, &limits);
    assert_eq!(
        counters.calls(0),
        1,
        "the base layer's own measurement must be served from `Stack`'s cache on a \
         second layout pass with no diff in between"
    );
}

#[test]
fn a_real_grid_reuses_its_track_cache_across_layout_calls_with_no_diff_in_between() {
    use taim_widget::grid;

    let counters = LayoutCounters::new(6);
    let renderer = headless_renderer();
    let limits = taim_core::layout::Limits::new(Size::ZERO, Size::new(300.0, 300.0));

    let mut element: Element<'static, ()> = grid((0..6).map(|i| leaf(i, 10.0, &counters)))
        .columns(3)
        .into();

    let mut tree = widget::Tree::new(element.as_widget());
    element.as_widget_mut().diff(&mut tree);

    let first = element
        .as_widget_mut()
        .layout(&mut tree, &renderer, &limits);
    let second = element
        .as_widget_mut()
        .layout(&mut tree, &renderer, &limits);

    // `Grid`'s track cache does not skip children's own `layout` calls
    // (each cell is still measured individually every pass -- only the
    // track geometry itself, i.e. cells-per-row/cell-size/row-count, is
    // cached), but repeating the same call under an unchanged `Track`
    // must still produce byte-identical geometry.
    assert_eq!(first.size(), second.size());
    assert_eq!(first.children().len(), second.children().len());

    for (a, b) in first.children().iter().zip(second.children()) {
        assert_eq!(a.bounds(), b.bounds());
    }
}
