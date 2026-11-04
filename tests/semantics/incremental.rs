//! Dirty-subtree correctness tests for `taim_runtime::semantics`,
//! against real `taim_core::Widget`s.
//!
//! `taim_runtime::semantics::build`'s own unit tests exercise a
//! synthetic arena; these tests additionally confirm the same
//! incremental-reuse behavior holds against real widgets going through
//! [`Widget::semantics`], [`Widget::visit_children`], and
//! `Tree::semantics_with_cache`, and that a change to one leaf's
//! reported semantics is visible in the assembled tree without forcing
//! every other leaf to recompute.
//!
//! These drive `semantics::Widgets` -- the same adapter
//! `UserInterface::semantics` uses -- rather than a hand-rolled one, so
//! what is tested here is what an application actually runs.

type Element<'a, M, T = taim_core::Theme, R = taim_renderer::Renderer> =
    taim_core::Element<'a, M, T, R>;
use std::cell::RefCell;
use std::rc::Rc;
use taim_core::Layout;
use taim_core::widget::tree::DirtyFlags;
use taim_core::widget::visit::{self, Visit};
use taim_core::widget::{self, Widget};
use taim_core::{Length, Rectangle, Size, Theme};
use taim_renderer::Renderer;
use taim_runtime::semantics;

fn headless_renderer() -> Renderer {
    taim_futures::futures::executor::block_on(<Renderer as taim_core::renderer::Headless>::new(
        taim_core::renderer::Settings::default(),
        Some("tiny-skia"),
    ))
    .expect("tiny-skia renderer must be available")
}

/// A leaf widget that reports a [`taim_core::semantics::Node`] naming
/// itself, and counts how many times it has been asked to.
struct Labeled {
    name: &'static str,
    calls: Rc<RefCell<Vec<&'static str>>>,
}

impl<Message> Widget<Message, Theme, Renderer> for Labeled {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(10.0), Length::Fixed(10.0))
    }

    fn layout(
        &mut self,
        _tree: &mut widget::Tree,
        _renderer: &Renderer,
        _limits: &taim_core::layout::Limits,
    ) -> taim_core::layout::Node {
        taim_core::layout::Node::new(Size::new(10.0, 10.0))
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &taim_core::renderer::Style,
        _layout: Layout<'_>,
        _cursor: taim_core::mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }

    fn semantics(
        &self,
        _tree: &widget::Tree,
        layout: Layout<'_>,
    ) -> Option<taim_core::semantics::Node> {
        self.calls.borrow_mut().push(self.name);

        Some(
            taim_core::semantics::Node::new(
                widget::Id::unique(),
                layout.bounds(),
                taim_core::semantics::Role::StaticText,
            )
            .with_name(self.name),
        )
    }
}

/// A container widget that reports no semantics of its own (the default
/// every unmodified widget gets), used to prove non-reporting containers
/// are transparent to `build`.
struct Group<Message> {
    children: Vec<Element<'static, Message>>,
}

impl<Message> Widget<Message, Theme, Renderer> for Group<Message> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
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
        taim_core::layout::Node::with_children(
            Size::new(10.0 * self.children.len() as f32, 10.0),
            self.children
                .iter_mut()
                .zip(tree.children.iter_mut())
                .enumerate()
                .map(|(index, (child, tree))| {
                    child
                        .as_widget_mut()
                        .layout(tree, renderer, limits)
                        .move_to(taim::Point::new(index as f32 * 10.0, 0.0))
                })
                .collect(),
        )
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &taim_core::renderer::Style,
        _layout: Layout<'_>,
        _cursor: taim_core::mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }

    fn visit_children(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        visit: &mut dyn FnMut(&mut dyn Visit),
    ) {
        self.children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .for_each(|((child, state), layout)| {
                visit(&mut visit::Widgets::new(
                    child.as_widget_mut(),
                    state,
                    layout,
                ));
            });
    }
}

fn labeled(name: &'static str, calls: &Rc<RefCell<Vec<&'static str>>>) -> Element<'static, ()> {
    Element::new(Labeled {
        name,
        calls: calls.clone(),
    })
}

fn group(children: Vec<Element<'static, ()>>) -> Element<'static, ()> {
    Element::new(Group { children })
}

/// Builds `root`, laying it out first so `Layout::bounds()`/children are
/// meaningful, and returns the `(Tree, layout::Node)` pair the caller
/// must keep alive alongside `root` for as long as a
/// [`semantics::Widgets`] borrows them.
fn layout_root(root: &mut Element<'_, ()>) -> (widget::Tree, taim_core::layout::Node) {
    let renderer = headless_renderer();
    let mut tree = widget::Tree::new(root.as_widget());
    tree.diff(root.as_widget_mut());

    let limits = taim_core::layout::Limits::new(Size::ZERO, Size::new(200.0, 200.0));
    let node = root.as_widget_mut().layout(&mut tree, &renderer, &limits);

    (tree, node)
}

/// Assembles `root`'s forest through the real adapter.
fn build(
    root: &mut Element<'_, ()>,
    tree: &mut widget::Tree,
    node: &taim_core::layout::Node,
) -> Vec<taim_core::semantics::Node> {
    semantics::build(&mut semantics::Widgets::new(
        root.as_widget_mut(),
        tree,
        Layout::new(node),
    ))
}

/// Names of every node in `forest`, depth-first.
fn names(forest: &[taim_core::semantics::Node]) -> Vec<&str> {
    fn recurse<'a>(nodes: &'a [taim_core::semantics::Node], out: &mut Vec<&'a str>) {
        for node in nodes {
            out.push(node.name.as_deref().unwrap());
            recurse(&node.children, out);
        }
    }

    let mut out = Vec::new();
    recurse(forest, &mut out);
    out
}

#[test]
fn a_single_reporting_leaf_produces_its_own_node() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut root = labeled("root", &calls);

    let (mut tree, node) = layout_root(&mut root);

    let built = build(&mut root, &mut tree, &node)
        .into_iter()
        .next()
        .expect("a Labeled widget always reports semantics");

    assert_eq!(built.name.as_deref(), Some("root"));
}

#[test]
fn a_non_reporting_container_is_transparent_to_its_reporting_children() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut root = group(vec![labeled("a", &calls), labeled("b", &calls)]);

    let (mut tree, node) = layout_root(&mut root);

    assert_eq!(
        names(&build(&mut root, &mut tree, &node)),
        vec!["a", "b"],
        "the reporting children must surface as a forest even though Group itself reports nothing"
    );
}

#[test]
fn a_deeply_nested_reporting_leaf_is_still_reached() {
    // Grandchildren, which the hand-rolled adapter these tests used to
    // carry could not reach at all: it handed every child an empty slice
    // of children, so anything below depth one was silently invisible.
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut root = group(vec![group(vec![group(vec![labeled("deep", &calls)])])]);

    let (mut tree, node) = layout_root(&mut root);

    assert_eq!(
        names(&build(&mut root, &mut tree, &node)),
        vec!["deep"],
        "a leaf three transparent containers down must still be hoisted to the root"
    );
}

#[test]
fn a_clean_leaf_does_not_recompute_its_semantics_on_a_second_build() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut root = labeled("root", &calls);

    let (mut tree, node) = layout_root(&mut root);

    let _ = build(&mut root, &mut tree, &node);
    calls.borrow_mut().clear();

    let _ = build(&mut root, &mut tree, &node);

    assert!(
        calls.borrow().is_empty(),
        "a clean leaf must reuse its cached semantics, not recompute"
    );
}

#[test]
fn marking_one_child_dirty_only_recomputes_that_childs_semantics() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut root = group(vec![labeled("a", &calls), labeled("b", &calls)]);

    let (mut tree, node) = layout_root(&mut root);

    let _ = build(&mut root, &mut tree, &node);
    calls.borrow_mut().clear();

    tree.children[0].mark_dirty(DirtyFlags::SEMANTICS);

    let built = build(&mut root, &mut tree, &node);

    assert_eq!(
        *calls.borrow(),
        vec!["a"],
        "only the marked child's own semantics should recompute"
    );
    assert_eq!(
        names(&built),
        vec!["a", "b"],
        "the clean sibling's cached result must still be present in the assembled tree"
    );
}
