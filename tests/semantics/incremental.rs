//! Dirty-subtree correctness tests for `iced_runtime::semantics`,
//! against real `iced_core::Widget`s.
//!
//! `iced_runtime::semantics::build`'s own unit tests exercise a
//! synthetic arena; these tests additionally confirm the same
//! incremental-reuse behavior holds against real widgets going through
//! [`Widget::semantics`] and `Tree::semantics_with_cache`, and that a
//! change to one leaf's reported semantics is visible in the assembled
//! tree without forcing every other leaf to recompute.

use iced::advanced::Layout;
use iced::advanced::widget::{self, Widget};
use iced::{Element, Length, Rectangle, Size, Theme};
use iced_core::widget::tree::DirtyFlags;
use iced_renderer::Renderer;
use iced_runtime::semantics::{self, Semantic};
use std::cell::RefCell;
use std::rc::Rc;

fn headless_renderer() -> Renderer {
    iced_futures::futures::executor::block_on(
        <Renderer as iced::advanced::renderer::Headless>::new(
            iced::advanced::renderer::Settings::default(),
            Some("tiny-skia"),
        ),
    )
    .expect("tiny-skia renderer must be available")
}

/// A leaf widget that reports a [`iced_core::semantics::Node`] naming
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
        _limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        iced::advanced::layout::Node::new(Size::new(10.0, 10.0))
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &iced::advanced::renderer::Style,
        _layout: Layout<'_>,
        _cursor: iced::advanced::mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }

    fn semantics(
        &self,
        _tree: &widget::Tree,
        layout: Layout<'_>,
    ) -> Option<iced_core::semantics::Node> {
        self.calls.borrow_mut().push(self.name);

        Some(
            iced_core::semantics::Node::new(
                widget::Id::unique(),
                layout.bounds(),
                iced_core::semantics::Role::StaticText,
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
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        iced::advanced::layout::Node::with_children(
            Size::new(10.0 * self.children.len() as f32, 10.0),
            self.children
                .iter_mut()
                .zip(tree.children.iter_mut())
                .enumerate()
                .map(|(index, (child, tree))| {
                    child
                        .as_widget_mut()
                        .layout(tree, renderer, limits)
                        .move_to(iced::Point::new(index as f32 * 10.0, 0.0))
                })
                .collect(),
        )
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &iced::advanced::renderer::Style,
        _layout: Layout<'_>,
        _cursor: iced::advanced::mouse::Cursor,
        _viewport: &Rectangle,
    ) {
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

/// Adapts a real `(widget, Tree, Layout)` triple to
/// `iced_runtime::semantics::Semantic`.
///
/// Holds a shared `&'a` borrow of the widget and layout (both read-only
/// for this purpose) plus a `RefCell<Tree>` for the interior mutability
/// `Tree::semantics_with_cache` needs -- the same shape
/// `iced_runtime::semantics`'s own unit tests use, for the same reason:
/// `Semantic::child` must be able to produce a fresh `Self` while the
/// caller already holds `&mut self` on the parent, which a `&mut
/// Tree`-holding adapter cannot do without an artificial lifetime
/// extension.
struct Adapter<'a> {
    widget: &'a dyn Widget<(), Theme, Renderer>,
    tree: &'a RefCell<widget::Tree>,
    layout: Layout<'a>,
    child_widgets: &'a [Element<'a, ()>],
    child_trees: &'a [RefCell<widget::Tree>],
}

impl<'a> Semantic for Adapter<'a> {
    fn node(&mut self) -> Option<iced_core::semantics::Node> {
        let widget = self.widget;
        let layout = self.layout;

        self.tree
            .borrow_mut()
            .semantics_with_cache(|tree| widget.semantics(tree, layout))
    }

    fn child_count(&self) -> usize {
        self.child_widgets.len()
    }

    fn child(&mut self, index: usize) -> Self {
        let child_layout = self
            .layout
            .children()
            .nth(index)
            .expect("index must be within child_count");

        Adapter {
            widget: self.child_widgets[index].as_widget(),
            tree: &self.child_trees[index],
            layout: child_layout,
            // Every widget under test here is at most two levels deep;
            // an empty slice keeps grandchildren untested, matching
            // `tests/events/hit_path.rs`'s leaf convention.
            child_widgets: &[],
            child_trees: &[],
        }
    }
}

/// Builds `root`, laying it out first so `Layout::bounds()`/children are
/// meaningful, and returns the `(Tree, layout::Node)` pair the caller
/// must keep alive alongside `root` for as long as an [`Adapter`]
/// borrows them.
fn layout_root(root: &mut Element<'_, ()>) -> (widget::Tree, iced::advanced::layout::Node) {
    let renderer = headless_renderer();
    let mut tree = widget::Tree::new(root.as_widget());
    tree.diff(root.as_widget_mut());

    let limits = iced::advanced::layout::Limits::new(Size::ZERO, Size::new(200.0, 200.0));
    let node = root.as_widget_mut().layout(&mut tree, &renderer, &limits);

    (tree, node)
}

#[test]
fn a_single_reporting_leaf_produces_its_own_node() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut root = labeled("root", &calls);

    let (tree, node) = layout_root(&mut root);
    let tree = RefCell::new(tree);
    let layout = Layout::new(&node);

    let built = semantics::build(&mut Adapter {
        widget: root.as_widget(),
        tree: &tree,
        layout,
        child_widgets: &[],
        child_trees: &[],
    });
    let built = built
        .into_iter()
        .next()
        .expect("a Labeled widget always reports semantics");

    assert_eq!(built.name.as_deref(), Some("root"));
}

#[test]
fn a_non_reporting_container_is_transparent_to_its_reporting_children() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let children = [labeled("a", &calls), labeled("b", &calls)];
    let mut root = group(vec![labeled("a", &calls), labeled("b", &calls)]);

    let (mut tree, node) = layout_root(&mut root);
    let child_trees: Vec<RefCell<widget::Tree>> =
        tree.children.drain(..).map(RefCell::new).collect();
    let tree = RefCell::new(tree);
    let layout = Layout::new(&node);

    let built = semantics::build(&mut Adapter {
        widget: root.as_widget(),
        tree: &tree,
        layout,
        child_widgets: &children,
        child_trees: &child_trees,
    });

    assert_eq!(
        built
            .iter()
            .map(|node| node.name.as_deref().unwrap())
            .collect::<Vec<_>>(),
        vec!["a", "b"],
        "the reporting children must surface as a forest even though Group itself reports nothing"
    );
}

#[test]
fn a_clean_leaf_does_not_recompute_its_semantics_on_a_second_build() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut root = labeled("root", &calls);

    let (tree, node) = layout_root(&mut root);
    let tree = RefCell::new(tree);
    let layout = Layout::new(&node);

    let adapter = || Adapter {
        widget: root.as_widget(),
        tree: &tree,
        layout,
        child_widgets: &[],
        child_trees: &[],
    };

    let _ = semantics::build(&mut adapter());
    calls.borrow_mut().clear();

    let _ = semantics::build(&mut adapter());

    assert!(
        calls.borrow().is_empty(),
        "a clean leaf must reuse its cached semantics, not recompute"
    );
}

#[test]
fn marking_one_child_dirty_only_recomputes_that_childs_semantics() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let children = [labeled("a", &calls), labeled("b", &calls)];
    let mut root = group(vec![labeled("a", &calls), labeled("b", &calls)]);

    let (mut tree, node) = layout_root(&mut root);
    let child_trees: Vec<RefCell<widget::Tree>> =
        tree.children.drain(..).map(RefCell::new).collect();
    let tree = RefCell::new(tree);
    let layout = Layout::new(&node);

    let adapter = || Adapter {
        widget: root.as_widget(),
        tree: &tree,
        layout,
        child_widgets: &children,
        child_trees: &child_trees,
    };

    let _ = semantics::build(&mut adapter());
    calls.borrow_mut().clear();

    child_trees[0]
        .borrow_mut()
        .mark_dirty(DirtyFlags::SEMANTICS);

    let built = semantics::build(&mut adapter());

    assert_eq!(
        *calls.borrow(),
        vec!["a"],
        "only the marked child's own semantics should recompute"
    );
    assert_eq!(
        built
            .iter()
            .map(|node| node.name.as_deref().unwrap())
            .collect::<Vec<_>>(),
        vec!["a", "b"],
        "the clean sibling's cached result must still be present in the assembled tree"
    );
}
