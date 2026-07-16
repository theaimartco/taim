//! Constraint/revision matrix and clean-subtree assertions.
//!
//! This file proves the exit gate stated in `phase.md` Phase 3:
//! "changing one leaf in a 10,000-node synthetic tree lays out/paints
//! only the required path/subtree." It builds a synthetic tree of
//! `Leaf`/`Branch` test widgets that cooperate with
//! `iced_core::widget::tree::Tree::layout_with_cache` (added in this
//! phase's second commit), instruments every leaf's layout call with a
//! shared counter, and asserts that re-laying-out the tree after
//! touching exactly one leaf only re-measures that leaf and its
//! ancestors -- never a sibling subtree.
//!
//! This does **not** prove that iced's shipped, production widgets
//! (`Column`, `Row`, `Container`, ...) get this speedup today -- none of
//! them call `layout_with_cache` yet, since retrofitting every widget is
//! out of this phase's scope (see `docs/architecture/widget-lifecycle.md`).
//! It proves the mechanism itself is correct and safe to adopt.

use iced::advanced::widget::{self, Widget};
use iced::advanced::{Layout, mouse, renderer};
use iced::{Element, Length, Point, Rectangle, Size, Theme};
use iced_renderer::Renderer;

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
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        let counters = self.counters.clone();
        let index = self.index;
        let size = self.size;

        tree.layout_with_cache(*limits, move |_tree| {
            counters.record(index);
            iced::advanced::layout::Node::new(Size::new(size, size))
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
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
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

        iced::advanced::layout::Node::with_children(Size::new(width, height), positioned)
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
    iced_futures::futures::executor::block_on(
        <Renderer as iced::advanced::renderer::Headless>::new(
            iced::advanced::renderer::Settings::default(),
            Some("tiny-skia"),
        ),
    )
    .expect("tiny-skia renderer must be available")
}

#[test]
fn changing_one_leaf_only_recomputes_that_leaf() {
    let counters = LayoutCounters::new(LEAF_COUNT);
    let renderer = headless_renderer();
    let limits = iced::advanced::layout::Limits::new(Size::ZERO, Size::new(2000.0, 200_000.0));

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

    let limits_a = iced::advanced::layout::Limits::new(Size::ZERO, Size::new(2000.0, 200_000.0));
    let _ = root.as_widget_mut().layout(&mut tree, &renderer, &limits_a);

    assert_eq!(counters.total_calls(), LEAF_COUNT as u32);

    let limits_b = iced::advanced::layout::Limits::new(Size::ZERO, Size::new(1000.0, 100_000.0));
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
    let limits = iced::advanced::layout::Limits::new(Size::ZERO, Size::new(2000.0, 200_000.0));

    let mut root = build_tree(&counters);
    let mut tree = widget::Tree::new(root.as_widget());
    tree.diff(root.as_widget_mut());

    let _ = root.as_widget_mut().layout(&mut tree, &renderer, &limits);

    tree.children[0].children[0].mark_dirty(widget::tree::DirtyFlags::LAYOUT);
    tree.children[0].children[0].mark_dirty(widget::tree::DirtyFlags::LAYOUT);

    let _ = root.as_widget_mut().layout(&mut tree, &renderer, &limits);

    assert_eq!(counters.calls(0), 2);
}
